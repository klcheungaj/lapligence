//! Force.

use super::*;

fn force_target_address(ctx: &RCtx<'_>, signal: usize) -> (String, String) {
    let target = ctx.model.signal(signal);
    match target.net_driver {
        Some((group, _)) => {
            let name = &ctx.model.net_group(group).c_name;
            (format!("&{name}.resolved"), format!("&{name}"))
        }
        None if target.net_alias.len() == 1 => {
            let group = target.net_alias[0].group();
            let name = &ctx.model.net_group(group).c_name;
            (format!("&{name}.resolved"), format!("&{name}"))
        }
        None => (format!("&{}", target.c_name), "NULL".to_string()),
    }
}

fn constant_alias_bit(expr: &IrExpr) -> Option<u32> {
    let IrExprKind::Const(value) = expr.kind() else {
        return None;
    };
    if value.real_value().is_some()
        || value.fill().is_some()
        || value.x_mask().iter().any(|mask| *mask != 0)
        || value.z_mask().iter().any(|mask| *mask != 0)
        || value.bits().iter().skip(1).any(|limb| *limb != 0)
    {
        return None;
    }
    value.bits().first().copied()?.try_into().ok()
}

fn push_alias_force_part(
    ctx: &RCtx<'_>,
    binding: &crate::sim::ir::IrNetAliasBinding,
    value_lsb: u32,
    two_state: bool,
    seen: &mut HashSet<(usize, u32)>,
    out: &mut Vec<String>,
) {
    if !seen.insert((binding.group(), binding.group_bit())) {
        return;
    }
    let group = &ctx.model.net_group(binding.group()).c_name;
    out.push(format!(
        "{{ &{group}.resolved, &{group}, {}, {}, 1, {value_lsb}, {} }}",
        binding.group_bit(),
        binding.group_bit(),
        two_state as u8,
    ));
}

fn force_part(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    value_lsb: u32,
    out: &mut Vec<String>,
) -> Result<(), String> {
    match lhs {
        IrLhs::PackedSelect { .. } => {
            return Err("packed activation selects require structured owned emission".to_owned())
        }
        IrLhs::Whole(index) => {
            let signal = ctx.model.signal(*index);
            if matches!(signal.ty, IrType::Real { .. }) {
                return Err("real force target cannot be a packed force part".to_string());
            }
            if !signal.net_alias.is_empty() {
                let mut seen = HashSet::new();
                for binding in &signal.net_alias {
                    push_alias_force_part(
                        ctx,
                        binding,
                        value_lsb + binding.signal_bit(),
                        signal.ty.two_state(),
                        &mut seen,
                        out,
                    );
                }
                return Ok(());
            }
            let (target, net) = force_target_address(ctx, *index);
            let width = signal.ty.width();
            out.push(format!(
                "{{ {target}, {net}, {}, 0, {width}, {value_lsb}, {} }}",
                width.saturating_sub(1),
                signal.ty.two_state() as u8
            ));
        }
        IrLhs::WholeRef {
            addr,
            width,
            two_state,
            ..
        } => {
            out.push(format!(
                "{{ {addr}, NULL, {}, 0, {width}, {value_lsb}, {} }}",
                width.saturating_sub(1),
                *two_state as u8
            ));
        }
        IrLhs::Ref { .. } => {
            return Err("force target cannot be a ref formal".to_string());
        }
        IrLhs::Bit(index, select, two_state) => {
            let signal = ctx.model.signal(*index);
            if !signal.net_alias.is_empty() {
                let bit = constant_alias_bit(select).ok_or_else(|| {
                    "selected force on a net alias requires a constant index".to_string()
                })?;
                let binding = signal
                    .net_alias
                    .iter()
                    .find(|binding| binding.signal_bit() == bit)
                    .ok_or_else(|| {
                        "selected force on a net alias has an unmapped bit".to_string()
                    })?;
                let mut seen = HashSet::new();
                push_alias_force_part(
                    ctx,
                    binding,
                    value_lsb,
                    signal.ty.two_state() || *two_state,
                    &mut seen,
                    out,
                );
                return Ok(());
            }
            let (target, net) = force_target_address(ctx, *index);
            let select = render_expr(ctx, select)?.code;
            out.push(format!(
                "{{ {target}, {net}, (int64_t)sv4_to_i64({select}), (int64_t)sv4_to_i64({select}), 1, {value_lsb}, {} }}",
                (signal.ty.two_state() || *two_state) as u8
            ));
        }
        IrLhs::Part(index, left, right, two_state) => {
            let signal = ctx.model.signal(*index);
            if !signal.net_alias.is_empty() {
                let width = left.abs_diff(*right) as u32 + 1;
                let step = if left <= right { 1 } else { -1 };
                let mut seen = HashSet::new();
                for offset in 0..width {
                    let bit = left + i64::from(offset) * step;
                    let Some(binding) = signal.net_alias.iter().find(|binding| {
                        binding.signal_bit() == u32::try_from(bit).ok().unwrap_or(u32::MAX)
                    }) else {
                        return Err("selected force on a net alias has an unmapped bit".to_string());
                    };
                    push_alias_force_part(
                        ctx,
                        binding,
                        value_lsb + width - 1 - offset,
                        signal.ty.two_state() || *two_state,
                        &mut seen,
                        out,
                    );
                }
                return Ok(());
            }
            let (target, net) = force_target_address(ctx, *index);
            let width = left.abs_diff(*right) as u32 + 1;
            out.push(format!(
                "{{ {target}, {net}, {left}, {right}, {width}, {value_lsb}, {} }}",
                (signal.ty.two_state() || *two_state) as u8
            ));
        }
        IrLhs::IdxPart(..) => {
            return Err("indexed part-select force target reached emission".to_string())
        }
        IrLhs::ArrayElem { .. } => return Err("array force target reached emission".to_string()),
        IrLhs::Stream {
            parts,
            width,
            slice: _,
            direction: _,
        } => {
            let mut cursor = *width;
            for (part, part_width) in parts {
                cursor = cursor
                    .checked_sub(*part_width)
                    .ok_or_else(|| "force stream part widths exceed target width".to_string())?;
                force_part(ctx, part, value_lsb + cursor, out)?;
            }
        }
    }
    Ok(())
}

fn force_reads(ctx: &RCtx<'_>, reads: &[usize]) -> Result<(String, usize), String> {
    if reads.is_empty() {
        return Ok(("NULL".to_string(), 0));
    }
    let mut entries = Vec::with_capacity(reads.len());
    for index in reads {
        let signal = ctx
            .model
            .signals
            .get(*index)
            .ok_or_else(|| format!("force dependency signal {index} is out of bounds"))?;
        let entry = match signal.ty {
            IrType::Real { .. } => format!("{{ NULL, &{}, 1 }}", signal.c_name),
            IrType::Packed { .. } => {
                let pointer = if signal.net_alias.is_empty() {
                    format!("&{}", signal.c_name)
                } else {
                    format!("&llg_net_alias_{}.visible", index)
                };
                format!("{{ {pointer}, NULL, 0 }}")
            }
        };
        entries.push(entry);
    }
    Ok((
        format!("(llg_force_read_t[]){{ {} }}", entries.join(", ")),
        reads.len(),
    ))
}

pub(super) fn render_force(
    ctx: &RCtx<'_>,
    lhs: &IrLhs,
    eval: &str,
    reads: &[usize],
) -> Result<String, String> {
    let (read_ptr, read_count) = force_reads(ctx, reads)?;
    if let IrLhs::Whole(index) = lhs {
        if matches!(ctx.model.signal(*index).ty, IrType::Real { .. }) {
            return Ok(format!(
                "    llg_force_real(&{}, {eval}, {read_ptr}, {read_count});\n",
                ctx.model.signal(*index).c_name
            ));
        }
    }
    let mut parts = Vec::new();
    force_part(ctx, lhs, 0, &mut parts)?;
    if parts.is_empty() {
        return Err("force target has no packed parts".to_string());
    }
    let (slice, reverse) = match lhs {
        IrLhs::Stream {
            slice, direction, ..
        } => (*slice, matches!(direction, IrStreamDirection::RightToLeft)),
        _ => (0, false),
    };
    let reads = if read_count == 0 {
        "NULL".to_string()
    } else {
        read_ptr
    };
    Ok(format!(
        "    {{ llg_force_part_t _force_parts[] = {{ {} }}; llg_force_expr_parts(_force_parts, {}, {slice}, {}, {eval}, {reads}, {read_count}); }}\n",
        parts.join(", "),
        parts.len(),
        reverse as u8
    ))
}

pub(super) fn render_release(ctx: &RCtx<'_>, lhs: &IrLhs) -> Result<String, String> {
    if let IrLhs::Whole(index) = lhs {
        if matches!(ctx.model.signal(*index).ty, IrType::Real { .. }) {
            return Ok(format!(
                "    llg_release_real(&{});\n",
                ctx.model.signal(*index).c_name
            ));
        }
    }
    let mut parts = Vec::new();
    force_part(ctx, lhs, 0, &mut parts)?;
    if parts.is_empty() {
        return Err("release target has no packed parts".to_string());
    }
    let (slice, reverse) = match lhs {
        IrLhs::Stream {
            slice, direction, ..
        } => (*slice, matches!(direction, IrStreamDirection::RightToLeft)),
        _ => (0, false),
    };
    Ok(format!(
        "    {{ llg_force_part_t _release_parts[] = {{ {} }}; llg_release_parts(_release_parts, {}, {slice}, {}); }}\n",
        parts.join(", "),
        parts.len(),
        reverse as u8
    ))
}
