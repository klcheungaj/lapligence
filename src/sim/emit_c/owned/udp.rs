//! Shared immutable UDP tables and synchronous borrowed evaluation.
//!
//! A definition with at most [`UDP_DENSE_INDEX_MAX_INPUTS`] inputs is emitted
//! as a dense index holding one output state per input-state combination.
//! Input `k` contributes its 0/1/X state (Z reads as X) as base-3 digit `k`,
//! most significant first. An evaluation reads each input's state in place
//! and performs one table load, independent of the row count. Wider
//! definitions keep the source-order row masks and the runtime row scan.
use super::*;

/// Largest input count that gets a dense `3^n`-byte index. IEEE 1364-2001
/// 8.1.4 requires implementations to accept at least 10 combinational inputs,
/// so every portable definition is dense; at 10 inputs the index is 59,049
/// bytes, emitted once per definition however many instances share it.
pub(in crate::sim::emit_c) const UDP_DENSE_INDEX_MAX_INPUTS: usize = 10;

/// Output state of a combination no row matches (`sv4_fill` code for X).
const UDP_UNMATCHED: u8 = IrUdpOutput::Unknown as u8;

/// Dense output index of a definition small enough for one, else `None`.
/// The first matching source row wins; combinations with no row are X.
pub(in crate::sim::emit_c) fn dense_index(table: &IrUdpTable) -> Option<Vec<u8>> {
    if table.input_count == 0 || table.input_count > UDP_DENSE_INDEX_MAX_INPUTS {
        return None;
    }
    let size = 3usize.pow(u32::try_from(table.input_count).ok()?);
    let mut index: Vec<Option<u8>> = vec![None; size];
    let mut unclaimed = size;
    for row in &table.rows {
        if unclaimed == 0 {
            break;
        }
        // Positions matched by the row, built one input digit at a time.
        let mut positions = vec![0usize];
        for mask in &row.inputs {
            let states = (0..3usize).filter(|state| (*mask as u8) & (1 << state) != 0);
            positions = states
                .flat_map(|state| positions.iter().map(move |prefix| prefix * 3 + state))
                .collect();
        }
        for position in positions {
            if index[position].is_none() {
                index[position] = Some(row.output as u8);
                unclaimed -= 1;
            }
        }
    }
    Some(
        index
            .into_iter()
            .map(|state| state.unwrap_or(UDP_UNMATCHED))
            .collect(),
    )
}

pub(in crate::sim::emit_c) fn tables(model: &IrModel, out: &mut String) {
    if model
        .udp_tables
        .iter()
        .any(|table| table.input_count <= UDP_DENSE_INDEX_MAX_INPUTS)
    {
        // Base-3 digit of one scalar state: Z (3) matches as X (2).
        out.push_str(
            "static inline size_t llg_udp_digit(unsigned state) { return state > 2u ? 2u : state; }\n",
        );
    }
    for (index, table) in model.udp_tables.iter().enumerate() {
        if let Some(dense) = dense_index(table) {
            out.push_str(&format!(
                "static const uint8_t llg_udp_index_{index}[{}] = {{\n",
                dense.len()
            ));
            for line in dense.chunks(64) {
                out.push_str("  ");
                for state in line {
                    out.push_str(&format!("{state},"));
                }
                out.push('\n');
            }
            out.push_str("};\n");
            continue;
        }
        out.push_str(&format!(
            "static const uint8_t llg_udp_table_{index}[] = {{\n"
        ));
        if table.rows.is_empty() {
            out.push_str("  0,\n");
        }
        for row in &table.rows {
            let fields = row
                .inputs
                .iter()
                .map(|mask| (*mask as u8).to_string())
                .chain(std::iter::once((row.output as u8).to_string()))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("  {fields},\n"));
        }
        out.push_str("};\n");
    }
}

/// Width/sign wrappers around a one-bit operand keep its bit-0 state.
fn state_source(input: &IrExpr) -> &IrExpr {
    match &input.kind {
        IrExprKind::Convert { a } | IrExprKind::Resize { a } | IrExprKind::CastToPacked { a }
            if a.width == 1 && a.fill.is_none() && !a.is_real() =>
        {
            state_source(a)
        }
        _ => input,
    }
}

/// A known, nonnegative constant bit offset.
fn constant_offset(expr: &IrExpr) -> Option<u64> {
    let IrExprKind::Const(constant) = &expr.kind else {
        return None;
    };
    let known = constant.real.is_none()
        && constant.x.iter().chain(&constant.z).all(|word| *word == 0)
        && constant.bits.iter().skip(1).all(|word| *word == 0);
    let value = constant.bits.first().copied().unwrap_or(0);
    let negative = constant.signed
        && (1..=u64::BITS).contains(&constant.width)
        && value >> (constant.width - 1) & 1 != 0;
    (known && !negative).then_some(value)
}

impl Frame<'_, '_> {
    pub(super) fn udp_eval(&mut self, table: usize, inputs: &[IrExpr]) -> Result<Value, String> {
        if self.ctx.model.udp_tables[table].input_count > UDP_DENSE_INDEX_MAX_INPUTS {
            return self.udp_scan(table, inputs);
        }
        // Inputs are read in order. They are borrowed only when none of them
        // can change another's storage; otherwise each becomes an owner.
        let borrow = inputs.iter().all(operands::stable_expression);
        let mut held = Vec::with_capacity(inputs.len());
        let mut position = String::new();
        for input in inputs {
            let source = state_source(input);
            // A constant bit of a borrowed base is read in place;
            // `llg_sv4_state` returns X beyond the base, like the select.
            let selected = match &source.kind {
                IrExprKind::BitSel { base, idx } => constant_offset(idx).map(|bit| (base, bit)),
                IrExprKind::PartSel { base, left, right } if left == right => {
                    u64::try_from(*right).ok().map(|bit| (base, bit))
                }
                _ => None,
            };
            let (value, bit) = match selected {
                Some((base, bit)) if borrow => (self.operand(base)?, bit),
                _ if borrow => (self.operand(source)?, 0),
                _ => (self.expression(source)?, 0),
            };
            let digit = format!("llg_udp_digit(llg_sv4_state({}, {bit}ULL))", value.code);
            position = if position.is_empty() {
                digit
            } else {
                format!("({position}) * 3u + {digit}")
            };
            held.push(value);
        }
        let result = self.value(
            format!("sv4_fill(llg_udp_index_{table}[{position}], 1, 0)"),
            1,
            false,
        );
        for value in held {
            self.discard(value);
        }
        Ok(result)
    }

    /// Source-order row scan for definitions above the dense-index limit.
    fn udp_scan(&mut self, table: usize, inputs: &[IrExpr]) -> Result<Value, String> {
        let definition = &self.ctx.model.udp_tables[table];
        let (rows, input_count) = (definition.rows.len(), definition.input_count);
        let mut values = Vec::with_capacity(inputs.len());
        for input in inputs {
            values.push(self.expression(input)?);
        }
        let addresses = values
            .iter()
            .map(|value| format!("&{}", value.code))
            .collect::<Vec<_>>()
            .join(", ");
        let result = self.value(
            format!(
                "sv4_udp_eval(llg_udp_table_{table}, {rows}, {input_count}, (const sv4_t* const[]){{{addresses}}})"
            ),
            1,
            false,
        );
        for value in values {
            self.discard(value);
        }
        Ok(result)
    }
}
