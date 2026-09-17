//! Force descriptors borrow only persistent targets; evaluators publish owners.
use super::*;

impl Frame<'_, '_> {
    fn force_parts(&mut self, lhs: &IrLhs, offset: u32, parts: &mut Vec<String>) -> Result<(), String> {
        if let IrLhs::Stream { parts: children, width, .. } = lhs {
            let mut cursor = *width;
            for (child, width) in children {
                cursor = cursor.checked_sub(*width).ok_or_else(|| "force stream width mismatch".to_owned())?;
                self.force_parts(child, offset.checked_add(cursor).ok_or_else(|| "force stream offset overflow".to_owned())?, parts)?;
            }
            return Ok(());
        }
        if let IrLhs::Whole(index) | IrLhs::Bit(index, ..) | IrLhs::Part(index, ..) = lhs {
            let signal = self.ctx.model.signal(*index);
            if !signal.net_alias.is_empty() {
                let selected: Vec<(u32, u32)> = match lhs {
                    IrLhs::Whole(_) => signal.net_alias.iter().map(|b| (b.signal_bit(), b.signal_bit())).collect(),
                    IrLhs::Bit(_, expression, _) => vec![(constant_alias_bit(expression).ok_or_else(||
                        "selected force on a net alias requires a constant index".to_owned())?, 0)],
                    IrLhs::Part(_, left, right, _) => {
                        let width = u32::try_from(left.abs_diff(*right) + 1).map_err(|_| "force width overflow".to_owned())?;
                        let step = if left <= right { 1 } else { -1 };
                        (0..width).map(|i| Ok((u32::try_from(left + i64::from(i) * step)
                            .map_err(|_| "selected force on a net alias has an unmapped bit".to_owned())?, width - 1 - i)))
                            .collect::<Result<Vec<_>, String>>()?
                    }
                    _ => unreachable!(),
                };
                let two_state = signal.ty.two_state() || match lhs { IrLhs::Bit(_, _, state) | IrLhs::Part(_, _, _, state) => *state, _ => false };
                let mut seen = std::collections::HashSet::new();
                for (bit, source) in selected {
                    let binding = signal.net_alias.iter().find(|b| b.signal_bit() == bit)
                        .ok_or_else(|| "selected force on a net alias has an unmapped bit".to_owned())?;
                    if !seen.insert((binding.group(), binding.group_bit())) { continue; }
                    let group = &self.ctx.model.net_group(binding.group()).c_name;
                    let bit = binding.group_bit();
                    let source = offset.checked_add(source).ok_or_else(|| "force offset overflow".to_owned())?;
                    parts.push(format!("{{ &{group}.resolved, &{group}, {bit}, {bit}, 1, {source}, {} }}", u8::from(two_state)));
                }
                return Ok(());
            }
        }
        let (address, net, left, right, width, two_state) = match lhs {
            IrLhs::Whole(index) | IrLhs::Bit(index, ..) | IrLhs::Part(index, ..) => {
                let signal = self.ctx.model.signal(*index);
                if !signal.net_alias.is_empty() { return Err(pending("true-net-alias force descriptors")); }
                if signal.ty.width() == 0 { return Err("real force cannot be part of a packed force".to_owned()); }
                let (address, net) = if let Some((group, _)) = signal.net_driver {
                    let name = &self.ctx.model.net_group(group).c_name;
                    (format!("&{name}.resolved"), format!("&{name}"))
                } else { (format!("&{}", signal.c_name), "NULL".to_owned()) };
                let (left, right, width, selected_two_state) = match lhs {
                    IrLhs::Bit(_, expression, two_state) => {
                        let value = self.expression(expression)?;
                        let index = self.scalar("int64_t", format!("sv4_to_i64({})", value.code));
                        self.discard(value);
                        (index.clone(), index, 1, *two_state)
                    }
                    IrLhs::Part(_, left, right, two_state) =>
                        (format!("{left}LL"), format!("{right}LL"), (left.abs_diff(*right) + 1) as u32, *two_state),
                    _ => ((signal.ty.width() - 1).to_string(), "0".to_owned(), signal.ty.width(), false),
                };
                (address, net, left, right, width, signal.ty.two_state() || selected_two_state)
            }
            IrLhs::WholeRef { addr, width, two_state, .. } => {
                let binding = self.address(addr)?;
                if binding.automatic || *width == 0 { return Err(pending("automatic or real indirect force targets")); }
                (binding.address, "NULL".to_owned(), (width - 1).to_string(), "0".to_owned(), *width, *two_state)
            }
            _ => return Err(pending("this force target descriptor")),
        };
        parts.push(format!("{{ {address}, {net}, {left}, {right}, {width}u, {offset}u, {} }}", u8::from(two_state)));
        Ok(())
    }

    pub(super) fn force_task(&mut self, lhs: &IrLhs, evaluator: Option<&str>, reads: &[usize]) -> Result<(), String> {
        let mut read_values = Vec::new();
        for index in reads {
            let signal = self.ctx.model.signal(*index);
            let pointer = if signal.net_alias.is_empty() { format!("&{}", signal.c_name) }
                else { format!("&llg_net_alias_{index}.visible") };
            read_values.push(if signal.ty.width() == 0 { format!("{{ NULL, {pointer}, 1 }}") }
                else { format!("{{ {pointer}, NULL, 0 }}") });
        }
        let read_ptr = if read_values.is_empty() { "NULL".to_owned() } else {
            let name = self.name("force_reads");
            self.line(format!("llg_force_read_t {name}[] = {{ {} }};", read_values.join(", ")));
            name
        };
        if let IrLhs::Whole(index) = lhs {
            let signal = self.ctx.model.signal(*index);
            if signal.ty.width() == 0 {
                self.line(if let Some(evaluator) = evaluator {
                    format!("llg_force_real(&{}, {evaluator}, {read_ptr}, {});", signal.c_name, reads.len())
                } else { format!("llg_release_real(&{});", signal.c_name) });
                return Ok(());
            }
        }
        let mut parts = Vec::new();
        self.force_parts(lhs, 0, &mut parts)?;
        if parts.is_empty() { return Err("force target has no packed parts".to_owned()); }
        let name = self.name("force_parts");
        self.line(format!("llg_force_part_t {name}[] = {{ {} }};", parts.join(", ")));
        let (slice, reverse) = match lhs {
            IrLhs::Stream { slice, direction, .. } => (*slice, *direction == IrStreamDirection::RightToLeft),
            _ => (0, false),
        };
        self.line(if let Some(evaluator) = evaluator {
            format!("llg_force_expr_parts({name}, {}, {slice}, {}, {evaluator}, {read_ptr}, {});", parts.len(), u8::from(reverse), reads.len())
        } else { format!("llg_release_parts({name}, {}, {slice}, {});", parts.len(), u8::from(reverse)) });
        Ok(())
    }
}

// Alias bit identity is elaborated; never render a selector twice.
fn constant_alias_bit(expr: &IrExpr) -> Option<u32> {
    let IrExprKind::Const(value) = expr.kind() else { return None; };
    if value.real_value().is_some() || value.fill().is_some()
        || value.x_mask().iter().any(|m| *m != 0) || value.z_mask().iter().any(|m| *m != 0)
        || value.bits().iter().skip(1).any(|m| *m != 0) { return None; }
    value.bits().first().copied()?.try_into().ok()
}
