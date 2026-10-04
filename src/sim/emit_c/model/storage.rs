//! Storage.

use super::*;

/// Signal globals plus collapsed inout-net group storage.
/// Render alias-view bindings as run-length `llg_net_alias_part_t` rows.
/// Bindings that map consecutive view bits onto consecutive bits of the same
/// group driver slot share one `{ net, slot, signal_lsb, group_lsb, bit_count }`
/// row, so a declared view costs one row per contiguous run rather than one
/// per bit. Returns the initializer list and its row count.
pub(super) fn render_alias_parts(
    model: &IrModel,
    bindings: &[crate::sim::ir::IrNetAliasBinding],
) -> (String, usize) {
    let mut ordered = bindings.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|binding| (binding.group, binding.slot, binding.signal_bit));
    let mut runs: Vec<(usize, usize, u32, u32, u32)> = Vec::new();
    for binding in ordered {
        if let Some(run) = runs.last_mut() {
            if run.0 == binding.group
                && run.1 == binding.slot
                && run.2.checked_add(run.4) == Some(binding.signal_bit)
                && run.3.checked_add(run.4) == Some(binding.group_bit)
            {
                run.4 += 1;
                continue;
            }
        }
        runs.push((
            binding.group,
            binding.slot,
            binding.signal_bit,
            binding.group_bit,
            1,
        ));
    }
    let parts = runs
        .iter()
        .map(|(group, slot, signal_lsb, group_lsb, width)| {
            format!(
                "{{ &{}, {slot}, {signal_lsb}, {group_lsb}, {width} }}",
                model.net_group(*group).c_name
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    (parts, runs.len())
}

pub(super) fn render_signal_decls(model: &IrModel, out: &mut String) {
    let mut emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for sig in &model.signals {
        if sig.net_driver.is_some()
            || sig.alias.is_some()
            || (sig.omit && sig.net_alias.is_empty())
            || !emitted.insert(sig.c_name.as_str())
        {
            // Net-group members: storage is emitted with its group; omitted
            // signals are pruned by `unused_storage`.
            continue;
        }
        match sig.ty {
            IrType::Real { .. } => out.push_str(&format!("double {} = 0.0;\n", sig.c_name)),
            IrType::Packed { .. } => {
                let init = "SV4_EMPTY";
                out.push_str(&format!("sv4_t {} = {init};\n", sig.c_name));
            }
        }
    }
    let mut groups_emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for g in &model.net_groups {
        if !groups_emitted.insert(g.c_name.as_str()) {
            continue;
        }
        let resolved_init = "SV4_EMPTY";
        let mut driver_ptrs = Vec::with_capacity(g.n_drivers);
        for slot in 0..g.n_drivers {
            driver_ptrs.push(format!("&{}__cells[{slot}]", g.c_name));
        }
        let strength0 = g
            .driver_strengths
            .iter()
            .map(|(zero, _)| zero.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let strength1 = g
            .driver_strengths
            .iter()
            .map(|(_, one)| one.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let (propagation_enabled, propagation_rise, propagation_fall, propagation_turn_off) =
            match g.propagation_delay {
                Some(delay) => (1, delay.rise, delay.fall, delay.turn_off),
                None => (0, 0, 0, 0),
            };
        // Exact elaborated-size driver and strength tables. The generated
        // `llg_net_t` points at them instead of embedding a fixed array, so
        // there is no artificial per-net driver ceiling.
        let (drivers_ptr, strength0_ptr, strength1_ptr, index_ptr, scratch_ptr) =
            if g.n_drivers == 0 {
                (
                    "NULL".to_owned(),
                    "NULL".to_owned(),
                    "NULL".to_owned(),
                    "NULL".to_owned(),
                    "NULL".to_owned(),
                )
            } else {
                out.push_str(&format!(
                    "sv4_t {}__cells[{}] = {{0}};\n\
                 static sv4_t* const {}__drivers[] = {{ {} }};\n\
                 static const uint8_t {}__strength0[] = {{ {} }};\n\
                 static const uint8_t {}__strength1[] = {{ {} }};\n\
                 static llg_net_driver_index_t {}__driver_index[{}];\n\
                 static int {}__overlap_scratch[{}];\n",
                    g.c_name,
                    g.n_drivers,
                    g.c_name,
                    driver_ptrs.join(", "),
                    g.c_name,
                    strength0,
                    g.c_name,
                    strength1,
                    g.c_name,
                    g.n_drivers,
                    g.c_name,
                    g.n_drivers,
                ));
                (
                    format!("{}__drivers", g.c_name),
                    format!("{}__strength0", g.c_name),
                    format!("{}__strength1", g.c_name),
                    format!("{}__driver_index", g.c_name),
                    format!("{}__overlap_scratch", g.c_name),
                )
            };
        let strength_view = g
            .strength_view
            .and_then(|view| model.signals.get(view))
            .filter(|signal| !signal.omit)
            .map_or_else(|| "NULL".to_owned(), |signal| format!("&{}", signal.c_name));
        out.push_str(&format!(
            "static llg_net_t {} = {{ .resolved = {resolved_init}, .width = {}, \
             .is_signed = {}, .resolution = {}, .n_drivers = {}, \
             .drivers = {drivers_ptr}, .strength0 = {strength0_ptr}, \
             .strength1 = {strength1_ptr}, .driver_index = {index_ptr}, \
             .overlap_scratch = {scratch_ptr}, .index_root = -1, \
             .propagation_enabled = {}, .propagation = NULL, \
             .propagation_rise = {}, .propagation_fall = {}, \
             .propagation_turn_off = {}, .aliases = NULL, \
             .strength = {strength_view} }};\n",
            g.c_name,
            g.width,
            g.signed as u8,
            g.kind.c_value(),
            g.n_drivers,
            propagation_enabled,
            propagation_rise,
            propagation_fall,
            propagation_turn_off,
        ));
    }
    // True net aliases retain their source storage for ABI/debug visibility,
    // while reads and sensitivity use the canonical resolved bits described by
    // these runtime descriptors.  Descriptors are emitted after all net
    // groups so every part can refer to its resolved group object.
    for (index, sig) in model.signals.iter().enumerate() {
        if sig.net_alias.is_empty() {
            continue;
        }
        let (parts, part_count) = render_alias_parts(model, &sig.net_alias);
        let visible = "SV4_EMPTY";
        out.push_str(&format!(
            "static const llg_net_alias_part_t llg_net_alias_{index}__parts[] = {{ {parts} }};\n\
             static llg_net_alias_t llg_net_alias_{index} = {{ &{}, {visible}, {}, {}, llg_net_alias_{index}__parts, {} }};\n",
            sig.c_name,
            sig.ty.width(),
            sig.ty.signed() as u8,
            part_count,
        ));
    }
    // Named events use a stable waiter-table object plus an assignable handle.
    for ev in &model.events {
        if ev.is_array() {
            continue;
        }
        out.push_str(&format!(
            "static llg_event_object_t {}__object = {{ 0 }};\n\
             static llg_event_t {} = {{ &{}__object }};\n",
            ev.c_name, ev.c_name, ev.c_name,
        ));
    }
    for ev in &model.events {
        let Some(dims) = ev.array_dims() else {
            continue;
        };
        let left = dims
            .iter()
            .map(|(left, _)| left.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let right = dims
            .iter()
            .map(|(_, right)| right.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let elements = ev
            .array_elements()
            .iter()
            .map(|index| format!("&{}", model.event(*index).c_name()))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!(
            "static const int32_t {}__left[] = {{ {} }};\n\
             static const int32_t {}__right[] = {{ {} }};\n\
             static llg_event_t* const {}__elements[] = {{ {} }};\n",
            ev.c_name(),
            left,
            ev.c_name(),
            right,
            ev.c_name(),
            elements
        ));
    }
}

/// Persistent subprogram locals are model storage, not C lexical locals. A
/// declaration initializer is applied by the typed initialization operation;
/// this declaration only supplies the language default before that operation.
pub(super) fn render_static_local_decls(model: &IrModel, out: &mut String) {
    let mut emitted = std::collections::HashSet::new();
    for function in &model.funcs {
        for local in &function.locals {
            if !emitted.insert(local.c_name()) {
                continue;
            }
            if local.string {
                out.push_str(&format!("llg_string_t {} = {{0}};\n", local.c_name()));
                continue;
            }
            if local.real {
                out.push_str(&format!("double {} = 0.0;\n", local.c_name()));
                continue;
            }
            let init = "SV4_EMPTY";
            out.push_str(&format!("sv4_t {} = {init};\n", local.c_name()));
        }
    }
}
