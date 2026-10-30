//! FSM coverage tables (IEEE 1800-2009 40.4, 40.5.3) from the FSMs the
//! owned database binds to each instance.
//!
//! Every FSM becomes one [`IrFsm`]: its state expression as storage bit
//! ranges and its states with their parameter values. The runtime watches
//! the state expression's storage, so the table exists only in models that
//! keep the coverage tables (a `$coverage_*` call or the VPI catalog); every
//! other model is generated without it and pays nothing.
use super::*;
use crate::sim::ir::{IrFsm, IrFsmExpression, IrFsmPart, IrFsmState, FSM_MAX_STATE_BITS};

impl Codegen<'_> {
    /// Reject a design whose FSM pragmas are malformed, misplaced or name
    /// nothing in their module: a misspelled pragma must not silently leave
    /// an FSM uncovered.
    pub(in crate::sim::codegen::lowering) fn check_fsm_pragmas(&self) -> Result<(), String> {
        let errors = self.db.fsm_pragma_errors();
        if errors.is_empty() {
            return Ok(());
        }
        Err(errors
            .iter()
            .map(|error| {
                format!(
                    "{}:{}:{}: {}",
                    error.file, error.line, error.column, error.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }

    /// Add the FSMs of every instance to the coverage tables of `model`.
    pub(in crate::sim::codegen::lowering) fn populate_fsm_coverage(
        &mut self,
        model: &mut IrModel,
    ) -> Result<(), String> {
        if self.db.fsms().is_empty() || model.assertion_coverage.is_none() {
            return Ok(());
        }
        let mut fsms = Vec::with_capacity(self.db.fsms().len());
        for fsm in self.db.fsms() {
            let Some(instance) = self.coverage_instances().index.get(&fsm.instance).copied() else {
                continue;
            };
            let path = self.instance_path_of(fsm.instance);
            let at = format!("{}:{}:{}", fsm.file, fsm.line, fsm.column);
            let mut parts = Vec::with_capacity(fsm.state.len());
            let mut width = 0u32;
            for part in &fsm.state {
                let signal = &self.node(part.signal).name;
                let Some(info) = self.sig_globals.get(&part.signal) else {
                    return Err(format!(
                        "{at}: FSM `{}` state signal `{signal}` has no storage",
                        fsm.name
                    ));
                };
                if info.real {
                    return Err(format!(
                        "{at}: FSM `{}` state signal `{signal}` must be a packed variable or net \
                         (IEEE 1800-2009 40.4.1)",
                        fsm.name
                    ));
                }
                let ir = info.ir;
                let total = info.width;
                if model
                    .signals
                    .get(ir)
                    .is_some_and(|signal| !signal.net_alias.is_empty())
                {
                    return Err(format!(
                        "{at}: unsupported: FSM `{}` state signal `{signal}` is an aliased net",
                        fsm.name
                    ));
                }
                let (lsb, bits) = match part.select {
                    None => (0, total),
                    Some((left, right)) => self
                        .fsm_select_bits(part.signal, total, left, right)
                        .ok_or_else(|| {
                            format!(
                                "{at}: FSM `{}` part-select `{signal}[{left}:{right}]` is not \
                                     within the declared range of `{signal}` in the declared \
                                     direction (IEEE 1800-2009 40.4.2)",
                                fsm.name
                            )
                        })?,
                };
                width = width.saturating_add(bits);
                parts.push(IrFsmPart {
                    signal: ir,
                    lsb,
                    width: bits,
                });
            }
            if width == 0 || width > FSM_MAX_STATE_BITS {
                return Err(format!(
                    "{at}: unsupported: FSM `{}` state vector is {width} bits wide; llg covers \
                     FSM state vectors of 1 to {FSM_MAX_STATE_BITS} bits",
                    fsm.name
                ));
            }
            let mut states: Vec<IrFsmState> = Vec::with_capacity(fsm.states.len());
            for &state in &fsm.states {
                let name = self.node(state).name.clone();
                let value = match self.kind(state) {
                    NodeKind::Param {
                        value: Some(Val::Bits(value)),
                        ..
                    } => value.to_u128(),
                    _ => None,
                };
                let Some(value) = value
                    .filter(|value| width >= 64 || *value >> width == 0)
                    .and_then(|value| u64::try_from(value).ok())
                else {
                    return Err(format!(
                        "{at}: FSM `{}` state `{name}` is not a known value of the {width}-bit \
                         state vector (IEEE 1800-2009 40.4.6)",
                        fsm.name
                    ));
                };
                if let Some(other) = states.iter().find(|other| other.value == value) {
                    return Err(format!(
                        "{at}: FSM `{}` states `{}` and `{name}` have the same value {value}",
                        fsm.name, other.name
                    ));
                }
                states.push(IrFsmState { name, value });
            }
            let expression = match fsm.state.as_slice() {
                [part] => {
                    let signal = format!("{path}.{}", self.node(part.signal).name);
                    match part.select {
                        None => IrFsmExpression::Signal(signal),
                        Some((left, right)) => IrFsmExpression::Select(signal, left, right),
                    }
                }
                _ => IrFsmExpression::Concatenation,
            };
            fsms.push(IrFsm {
                name: fsm.name.clone(),
                full_name: format!("{path}.{}", fsm.name),
                instance,
                width,
                parts,
                states,
                expression,
                file: fsm.file.clone(),
                line: fsm.line,
                column: fsm.column,
            });
        }
        if let Some(coverage) = model.assertion_coverage.as_mut() {
            coverage.fsms = fsms;
        }
        Ok(())
    }

    /// The storage offset and width of the part-select `[left:right]` of a
    /// one-dimensional vector `signal`, written in its declared direction.
    fn fsm_select_bits(
        &self,
        signal: NodeId,
        width: u32,
        left: i64,
        right: i64,
    ) -> Option<(u32, u32)> {
        let declared = match self.db.packed_dimensions(signal) {
            None => (i64::from(width) - 1, 0),
            Some([range]) => (
                i64::try_from(range.left).ok()?,
                i64::try_from(range.right).ok()?,
            ),
            Some(_) => return None,
        };
        let (msb, lsb) = declared;
        let descending = msb >= lsb;
        if (left >= right) != descending && left != right {
            return None;
        }
        let (low, high) = (msb.min(lsb), msb.max(lsb));
        if left < low || left > high || right < low || right > high {
            return None;
        }
        // Storage bit 0 is the declared right bound.
        let offset = if descending { right - lsb } else { lsb - right };
        let bits = (left - right).unsigned_abs() + 1;
        Some((u32::try_from(offset).ok()?, u32::try_from(bits).ok()?))
    }
}
