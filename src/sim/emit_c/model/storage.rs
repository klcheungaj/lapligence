//! Storage.

use super::*;

/// Signal globals plus collapsed inout-net group storage.
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
        let driver_init = "SV4_EMPTY";
        let resolved_init = "SV4_EMPTY";
        let mut driver_ptrs = Vec::with_capacity(g.n_drivers);
        for slot in 0..g.n_drivers {
            let cell = format!("{}_d{}", g.c_name, slot);
            out.push_str(&format!("sv4_t {cell} = {driver_init};\n"));
            driver_ptrs.push(format!("&{cell}"));
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
        let (drivers_ptr, strength0_ptr, strength1_ptr) = if g.n_drivers == 0 {
            ("NULL".to_owned(), "NULL".to_owned(), "NULL".to_owned())
        } else {
            out.push_str(&format!(
                "static sv4_t* const {}__drivers[] = {{ {} }};\n\
                 static const uint8_t {}__strength0[] = {{ {} }};\n\
                 static const uint8_t {}__strength1[] = {{ {} }};\n",
                g.c_name,
                driver_ptrs.join(", "),
                g.c_name,
                strength0,
                g.c_name,
                strength1,
            ));
            (
                format!("{}__drivers", g.c_name),
                format!("{}__strength0", g.c_name),
                format!("{}__strength1", g.c_name),
            )
        };
        out.push_str(&format!(
            "static llg_net_t {} = {{ {resolved_init}, {}, {}, {}, {}, {drivers_ptr}, {strength0_ptr}, {strength1_ptr}, {}, NULL, {}, {}, {}, 0, 0, NULL }};\n",
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
        let parts = sig
            .net_alias
            .iter()
            .map(|binding| {
                let group = &model.net_group(binding.group).c_name;
                format!(
                    "{{ &{group}, {}, {}, {} }}",
                    binding.slot, binding.signal_bit, binding.group_bit
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let visible = "SV4_EMPTY";
        out.push_str(&format!(
            "static const llg_net_alias_part_t llg_net_alias_{index}__parts[] = {{ {parts} }};\n\
             static llg_net_alias_t llg_net_alias_{index} = {{ &{}, {visible}, {}, {}, llg_net_alias_{index}__parts, {} }};\n",
            sig.c_name,
            sig.ty.width(),
            sig.ty.signed() as u8,
            sig.net_alias.len(),
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
