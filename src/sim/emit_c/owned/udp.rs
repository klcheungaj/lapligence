//! Shared immutable UDP tables and synchronous borrowed evaluation.
//!
//! A combinational definition with at most [`UDP_DENSE_INDEX_MAX_INPUTS`]
//! inputs is emitted as a dense index holding one output state per
//! input-state combination. Input `k` contributes its 0/1/X state (Z reads as
//! X) as base-3 digit `k`, most significant first. An evaluation reads each
//! input's state in place and performs one table load, independent of the
//! row count. Wider definitions keep the source-order row masks and the
//! runtime row scan.
//!
//! A sequential definition whose transition index has at most
//! [`UDP_SEQUENTIAL_DENSE_MAX_ENTRIES`] entries gets a dense next-state index
//! keyed by the changed input's transition slot, the input code after the
//! change and the current state (see [`sequential_index`]); each changed
//! input costs one load. Each instance owns a file-scope state record (the
//! prior input code and the output state) that one emitted step function per
//! definition advances. Larger definitions keep encoded rows and a scan in
//! their step function.
use super::*;

/// Largest input count that gets a dense `3^n`-byte index. IEEE 1364-2001
/// 8.1.4 requires implementations to accept at least 10 combinational inputs,
/// so every portable definition is dense; at 10 inputs the index is 59,049
/// bytes, emitted once per definition however many instances share it.
pub(in crate::sim::emit_c) const UDP_DENSE_INDEX_MAX_INPUTS: usize = 10;

/// Output state of a combination no row matches (`sv4_fill` code for X).
const UDP_UNMATCHED: u8 = IrUdpOutput::Unknown as u8;

/// Largest sequential next-state index, in one-byte entries, emitted per
/// definition. A definition with `n` inputs, `e` of them named with a
/// transition, needs `3^(n+1) * (1 + 2e)` entries: 1 MiB covers every
/// definition up to 9 inputs (the IEEE 1364-2001 8.1.2 sequential minimum)
/// with up to 8 edge inputs, and the index is shared by all instances.
pub(in crate::sim::emit_c) const UDP_SEQUENTIAL_DENSE_MAX_ENTRIES: usize = 1 << 20;

/// Base-3 code of `digits`, input 0 most significant.
fn udp_code(digits: &[u8]) -> usize {
    digits
        .iter()
        .fold(0, |code, digit| code * 3 + usize::from(*digit))
}

/// Every input code a row's level fields match, built one digit at a time.
fn level_codes(fields: &[IrUdpInput]) -> Vec<usize> {
    let mut codes = vec![0usize];
    for field in fields {
        let states = (0..3u8).filter(|state| field.matches(*state));
        codes = states
            .flat_map(|state| {
                codes
                    .iter()
                    .map(move |prefix| prefix * 3 + usize::from(state))
            })
            .collect();
    }
    codes
}

/// Dense next-state index of a sequential definition, or `None` above
/// [`UDP_SEQUENTIAL_DENSE_MAX_ENTRIES`].
///
/// Entry `(slot * 3^n + code) * 3 + state` holds the next state (0, 1, 2 =
/// X) when the inputs, after one changed input, have base-3 `code` and the
/// output holds `state`. Slot 0 serves a change of any input that no row
/// names with a transition: only level rows can match it. Edge input `e`
/// (ordinal among [`IrUdpSequential::edge_inputs`]) owns slots `1 + 2e` and
/// `2 + 2e`, for the old value below or above the new one (`old - (old >
/// new)`). Each entry is the first matching level row, else the first
/// matching edge row, else X ([`IrUdpSequential::next_state`]).
pub(in crate::sim::emit_c) fn sequential_index(
    input_count: usize,
    sequential: &IrUdpSequential,
) -> Option<Vec<u8>> {
    let edges = sequential.edge_inputs(input_count);
    let combinations = 3usize.checked_pow(u32::try_from(input_count).ok()?)?;
    let slots = 1 + 2 * edges.len();
    let entries = combinations.checked_mul(3)?.checked_mul(slots)?;
    if input_count == 0 || entries > UDP_SEQUENTIAL_DENSE_MAX_ENTRIES {
        return None;
    }
    let resolve = |row: &IrUdpSequentialRow, state: u8| match row.next {
        IrUdpNext::Output(output) => output as u8,
        IrUdpNext::Hold => state,
    };
    const UNSET: u8 = u8::MAX;
    // First matching level row per (code, state).
    let mut level = vec![UNSET; combinations * 3];
    for row in sequential.rows.iter().filter(|row| row.edge.is_none()) {
        for code in level_codes(&row.inputs) {
            for state in (0..3u8).filter(|state| row.state.matches(*state)) {
                let entry = &mut level[code * 3 + usize::from(state)];
                if *entry == UNSET {
                    *entry = resolve(row, state);
                }
            }
        }
    }
    let mut index = vec![UDP_UNMATCHED; entries];
    for (slot, entry) in index[..combinations * 3].iter_mut().enumerate() {
        if level[slot] != UNSET {
            *entry = level[slot];
        }
    }
    let place = 3usize.pow(u32::try_from(input_count).ok()?);
    for (ordinal, input) in edges.iter().enumerate() {
        // Digit weight of this input in the code.
        let weight = place / 3usize.pow(u32::try_from(input + 1).ok()?);
        let mut edge = vec![UNSET; 2 * combinations * 3];
        for row in &sequential.rows {
            let Some(transition) = row.edge.filter(|edge| edge.input == *input) else {
                continue;
            };
            for code in level_codes(&row.inputs) {
                let new = (code / weight % 3) as u8;
                for old in (0..3u8).filter(|old| *old != new && transition.from.matches(*old)) {
                    let relative = usize::from(old - u8::from(old > new));
                    for state in (0..3u8).filter(|state| row.state.matches(*state)) {
                        let entry =
                            &mut edge[(relative * combinations + code) * 3 + usize::from(state)];
                        if *entry == UNSET {
                            *entry = resolve(row, state);
                        }
                    }
                }
            }
        }
        for relative in 0..2 {
            let base = (1 + 2 * ordinal + relative) * combinations * 3;
            for position in 0..combinations * 3 {
                let level = level[position];
                let edge = edge[relative * combinations * 3 + position];
                index[base + position] = if level != UNSET {
                    level
                } else if edge != UNSET {
                    edge
                } else {
                    UDP_UNMATCHED
                };
            }
        }
    }
    Some(index)
}

/// Row encoding of a sequential definition above the dense limit, scanned
/// by its step function: per row `[edge, state, next]` then a
/// `(to, from)` mask pair per input, where `edge` is 1 for an edge row,
/// `next` 3 means no change, and `from` is zero except at the edge input.
fn sequential_rows(input_count: usize, sequential: &IrUdpSequential) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(sequential.rows.len() * (3 + 2 * input_count));
    for row in &sequential.rows {
        encoded.push(u8::from(row.edge.is_some()));
        encoded.push(row.state as u8);
        encoded.push(match row.next {
            IrUdpNext::Output(output) => output as u8,
            IrUdpNext::Hold => 3,
        });
        for (input, field) in row.inputs.iter().enumerate() {
            encoded.push(*field as u8);
            encoded.push(
                row.edge
                    .filter(|edge| edge.input == input)
                    .map_or(0, |edge| edge.from as u8),
            );
        }
    }
    encoded
}

fn byte_table(out: &mut String, name: &str, bytes: &[u8]) {
    out.push_str(&format!(
        "static const uint8_t {name}[{}] = {{\n",
        bytes.len().max(1)
    ));
    if bytes.is_empty() {
        out.push_str("  0,\n");
    }
    for line in bytes.chunks(64) {
        out.push_str("  ");
        for byte in line {
            out.push_str(&format!("{byte},"));
        }
        out.push('\n');
    }
    out.push_str("};\n");
}

/// The step function and state type of sequential definition `index`.
///
/// The dense step folds each input's digit into the stored code in port
/// order; a digit that changed is one transition, looked up with the code
/// in which earlier inputs already hold their new values (decision S3-D2).
/// The state record keeps the prior code and the output state.
fn sequential_step(
    out: &mut String,
    index: usize,
    table: &IrUdpTable,
    sequential: &IrUdpSequential,
) {
    let inputs = table.input_count;
    let parameters = (0..inputs)
        .map(|input| format!("size_t d{input}"))
        .collect::<Vec<_>>()
        .join(", ");
    if let Some(dense) = sequential_index(inputs, sequential) {
        byte_table(out, &format!("llg_udp_next_{index}"), &dense);
        let combinations = 3usize.pow(inputs as u32);
        let edges = sequential.edge_inputs(inputs);
        out.push_str(&format!(
            "static inline void llg_udp_step_{index}(llg_udp_seq_t* s, {parameters}) {{\n  size_t code = s->inputs;\n  size_t state = s->state, was;\n"
        ));
        for input in 0..inputs {
            let weight = 3usize.pow((inputs - 1 - input) as u32);
            let slot = match edges.iter().position(|edge| *edge == input) {
                Some(ordinal) => format!("{}u + was - (was > d{input})", 1 + 2 * ordinal),
                None => "0u".to_owned(),
            };
            out.push_str(&format!(
                "  was = code / {weight}u % 3u;\n  if (d{input} != was) {{\n    code = code - was * {weight}u + d{input} * {weight}u;\n    state = llg_udp_next_{index}[(({slot}) * {combinations}u + code) * 3u + state];\n  }}\n"
            ));
        }
        out.push_str("  s->inputs = (uint32_t)code;\n  s->state = (uint8_t)state;\n}\n");
        return;
    }
    byte_table(
        out,
        &format!("llg_udp_rows_{index}"),
        &sequential_rows(inputs, sequential),
    );
    let rows = sequential.rows.len();
    let stride = 3 + 2 * inputs;
    out.push_str(&format!(
        "typedef struct {{ uint8_t inputs[{inputs}]; uint8_t state; }} llg_udp_wide_{index}_t;\n\
         static inline void llg_udp_step_{index}(llg_udp_wide_{index}_t* s, {parameters}) {{\n\
         \x20 const size_t now[{inputs}] = {{{arguments}}};\n\
         \x20 size_t state = s->state;\n\
         \x20 for (size_t k = 0; k < {inputs}u; ++k) {{\n\
         \x20   const size_t was = s->inputs[k];\n\
         \x20   if (now[k] == was) continue;\n\
         \x20   s->inputs[k] = (uint8_t)now[k];\n\
         \x20   size_t next = 2u;\n\
         \x20   int found = 0;\n\
         \x20   /* Level rows decide first (IEEE 1364-2001 8.7), then edge rows on k. */\n\
         \x20   for (int pass = 0; pass < 2 && !found; ++pass) {{\n\
         \x20     for (size_t r = 0; r < {rows}u && !found; ++r) {{\n\
         \x20       const uint8_t* row = llg_udp_rows_{index} + r * {stride}u;\n\
         \x20       if (row[0] != (uint8_t)pass || !(row[1] & (1u << state))) continue;\n\
         \x20       if (pass == 1 && !(row[4 + 2 * k] & (1u << was))) continue;\n\
         \x20       size_t j = 0;\n\
         \x20       while (j < {inputs}u && (row[3 + 2 * j] & (1u << s->inputs[j]))) ++j;\n\
         \x20       if (j < {inputs}u) continue;\n\
         \x20       next = row[2] == 3u ? state : row[2];\n\
         \x20       found = 1;\n\
         \x20     }}\n\
         \x20   }}\n\
         \x20   state = next;\n\
         \x20 }}\n\
         \x20 s->state = (uint8_t)state;\n\
         }}\n",
        arguments = (0..inputs)
            .map(|input| format!("d{input}"))
            .collect::<Vec<_>>()
            .join(", "),
    ));
}

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
        .any(|table| table.sequential.is_some() || table.input_count <= UDP_DENSE_INDEX_MAX_INPUTS)
    {
        // Base-3 digit of one scalar state: Z (3) matches as X (2).
        out.push_str(
            "static inline size_t llg_udp_digit(unsigned state) { return state > 2u ? 2u : state; }\n",
        );
    }
    if !model.udp_states.is_empty() {
        out.push_str("typedef struct { uint32_t inputs; uint8_t state; } llg_udp_seq_t;\n");
    }
    for (index, table) in model.udp_tables.iter().enumerate() {
        if let Some(sequential) = &table.sequential {
            sequential_step(out, index, table, sequential);
            continue;
        }
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
    // One state record per instance. Simulation starts with every input
    // unknown (code all X) and the output at its initial value, else X.
    for (state, table) in model.udp_states.iter().enumerate() {
        let definition = &model.udp_tables[*table];
        let Some(sequential) = &definition.sequential else {
            continue;
        };
        let initial = sequential.initial as u8;
        if dense_sequential(definition) {
            let all_unknown = udp_code(&vec![2; definition.input_count]);
            out.push_str(&format!(
                "static llg_udp_seq_t llg_udp_state_{state} = {{ {all_unknown}u, {initial}u }};\n"
            ));
        } else {
            let unknown = vec!["2"; definition.input_count].join(", ");
            out.push_str(&format!(
                "static llg_udp_wide_{table}_t llg_udp_state_{state} = {{ {{ {unknown} }}, {initial}u }};\n"
            ));
        }
    }
}

/// The C type of sequential instance `state`'s record, `None` for a
/// combinational instance, which keeps no state.
pub(in crate::sim::emit_c) fn state_type(model: &IrModel, state: usize) -> Option<String> {
    let table = *model.udp_states.get(state)?;
    let definition = &model.udp_tables[table];
    definition.sequential.as_ref()?;
    Some(if dense_sequential(definition) {
        "llg_udp_seq_t".to_owned()
    } else {
        format!("llg_udp_wide_{table}_t")
    })
}

/// Whether sequential `table` uses the dense index (cheap size check).
fn dense_sequential(table: &IrUdpTable) -> bool {
    table.sequential.as_ref().is_some_and(|sequential| {
        let edges = sequential.edge_inputs(table.input_count).len();
        u32::try_from(table.input_count)
            .ok()
            .and_then(|count| 3usize.checked_pow(count + 1))
            .and_then(|size| size.checked_mul(1 + 2 * edges))
            .is_some_and(|entries| {
                table.input_count > 0 && entries <= UDP_SEQUENTIAL_DENSE_MAX_ENTRIES
            })
    })
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
    /// Read each scalar input's 0/1/X digit in order (Z reads as X). Inputs
    /// are borrowed in place only when none of them can change another's
    /// storage; otherwise each becomes an owner. Returns the digit
    /// expressions and the values to discard after their use.
    fn udp_digits(&mut self, inputs: &[IrExpr]) -> Result<(Vec<String>, Vec<Value>), String> {
        let borrow = inputs.iter().all(operands::stable_expression);
        let mut held = Vec::with_capacity(inputs.len());
        let mut digits = Vec::with_capacity(inputs.len());
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
            digits.push(format!(
                "llg_udp_digit(llg_sv4_state({}, {bit}ULL))",
                value.code
            ));
            held.push(value);
        }
        Ok((digits, held))
    }

    /// Advance one sequential instance's state from its current inputs.
    pub(super) fn udp_step(&mut self, state: usize, inputs: &[IrExpr]) -> Result<(), String> {
        let table = self.ctx.model.udp_states[state];
        let (digits, held) = self.udp_digits(inputs)?;
        self.line(format!(
            "llg_udp_step_{table}(&llg_udp_state_{state}, {});",
            digits.join(", ")
        ));
        for value in held {
            self.discard(value);
        }
        Ok(())
    }

    /// The output of one sequential instance, as its last step left it.
    pub(super) fn udp_state(&mut self, state: usize) -> Value {
        self.value(
            format!("sv4_fill(llg_udp_state_{state}.state, 1, 0)"),
            1,
            false,
        )
    }

    pub(super) fn udp_eval(&mut self, table: usize, inputs: &[IrExpr]) -> Result<Value, String> {
        if self.ctx.model.udp_tables[table].input_count > UDP_DENSE_INDEX_MAX_INPUTS {
            return self.udp_scan(table, inputs);
        }
        let (digits, held) = self.udp_digits(inputs)?;
        let mut position = String::new();
        for digit in digits {
            position = if position.is_empty() {
                digit
            } else {
                format!("({position}) * 3u + {digit}")
            };
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
