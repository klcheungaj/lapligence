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
    // Module path destinations publish every bit through its own inertial
    // record; every other net keeps its initializer unchanged.
    let path_destinations = model
        .module_paths()
        .iter()
        .flat_map(|set| set.destinations())
        .map(|destination| destination.group())
        .collect::<std::collections::HashSet<_>>();
    for (group_index, g) in model.net_groups.iter().enumerate() {
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
        let path_destination = path_destinations.contains(&group_index);
        let (propagation_enabled, propagation_rise, propagation_fall, propagation_turn_off) =
            match g.propagation_delay {
                Some(delay) => (1, delay.rise, delay.fall, delay.turn_off),
                None => (u8::from(path_destination), 0, 0, 0),
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
        // A path destination keeps one path-stage inertial handle per bit.
        let mut propagation_bits = if path_destination {
            out.push_str(&format!(
                "static llg_inertial_t* {}__propagation_bits[{}];\n",
                g.c_name, g.width
            ));
            format!(".propagation_bits = {}__propagation_bits, ", g.c_name)
        } else {
            String::new()
        };
        // A delayed net driven in separate parts schedules each part on its
        // own (S2-D3); a net scheduled whole leaves these fields zero.
        if g.propagation_delay.is_some() && !g.propagation_segments.is_empty() {
            let bounds = std::iter::once(0)
                .chain(g.propagation_segments.iter().copied())
                .chain(std::iter::once(g.width))
                .map(|bound| bound.to_string())
                .collect::<Vec<_>>();
            out.push_str(&format!(
                "static const uint32_t {name}__segment_lsb[] = {{ {} }};\n\
                 static llg_inertial_t* {name}__segment_records[{}];\n",
                bounds.join(", "),
                bounds.len() - 1,
                name = g.c_name,
            ));
            propagation_bits.push_str(&format!(
                ".n_segments = {}, .segment_lsb = {name}__segment_lsb, \
                 .segment_records = {name}__segment_records, ",
                bounds.len() - 1,
                name = g.c_name,
            ));
        }
        if g.runtime_delay {
            propagation_bits.push_str(".net_delayed = 2, ");
        } else if g.propagation_delay.is_some() {
            propagation_bits.push_str(".net_delayed = 1, ");
        }
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
             .propagation_turn_off = {}, {propagation_bits}.aliases = NULL, \
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
    render_module_paths(model, out);
    render_switch_network(model, out);
    render_timing_checks(model, out);
    render_sdf_annotations(model, out);
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

/// Storage address of a signal after alias resolution, as the runtime reads
/// module path sources.
fn path_source_storage(model: &IrModel, index: usize) -> String {
    let mut index = index;
    let mut seen = std::collections::HashSet::new();
    while seen.insert(index) {
        let Some(alias) = model.signals.get(index).and_then(|signal| signal.alias) else {
            break;
        };
        index = alias;
    }
    let signal = model.signal(index);
    if signal.net_alias.is_empty() {
        format!("&{}", signal.c_name)
    } else {
        format!("&llg_net_alias_{index}.visible")
    }
}

/// Bidirectional switch tables (`llg_switch_node_t`, `llg_switch_t`), after
/// every net group they name; nothing for a design without switches. The
/// nodes of one connected network are adjacent.
fn render_switch_network(model: &IrModel, out: &mut String) {
    let network = model.switch_network();
    if network.is_empty() {
        return;
    }
    out.push_str("static const llg_switch_node_t llg_switch_nodes[] = {\n");
    for node in network.nodes() {
        out.push_str(&format!(
            "    {{ &{}, {}u }},\n",
            model.net_group(node.group()).c_name,
            node.bit()
        ));
    }
    out.push_str("};\nstatic const llg_switch_t llg_switches[] = {\n");
    for switch in network.switches() {
        let mut flags = Vec::new();
        if switch.resistive() {
            flags.push("LLG_SWITCH_RESISTIVE");
        }
        if switch.controlled() {
            flags.push("LLG_SWITCH_CONTROLLED");
        }
        let flags = if flags.is_empty() {
            "0".to_owned()
        } else {
            flags.join(" | ")
        };
        out.push_str(&format!(
            "    {{ {}u, {}u, {flags} }},\n",
            switch.a(),
            switch.b()
        ));
    }
    out.push_str("};\n");
}

/// Module path tables: one `llg_module_path_set_t` per instance with paths,
/// after every net group they name. Delay and limit tables are writable so
/// back-annotation can replace them. Each record ends with a designated
/// field; the other ADV-008 fields are emitted only when used, and nothing is
/// emitted for a design without paths. A set with path conditions points at
/// its generated evaluator, defined with the model's functions
/// (`render_path_condition_evaluators`).
fn render_module_paths(model: &IrModel, out: &mut String) {
    use crate::sim::ir::IrPathCondition;
    for (index, set) in model.module_paths().iter().enumerate() {
        let name = format!("llg_path_set_{index}");
        let sources = set
            .sources()
            .iter()
            .map(|signal| path_source_storage(model, *signal))
            .collect::<Vec<_>>();
        let sources = if sources.is_empty() {
            "NULL".to_owned()
        } else {
            out.push_str(&format!(
                "static sv4_t* const {name}__sources[] = {{ {} }};\n",
                sources.join(", ")
            ));
            format!("{name}__sources")
        };
        let mut dests = Vec::with_capacity(set.destinations().len());
        for (dest_index, destination) in set.destinations().iter().enumerate() {
            let mut rows = Vec::with_capacity(destination.paths().len());
            for (path_index, path) in destination.paths().iter().enumerate() {
                let delays = path
                    .delays()
                    .iter()
                    .map(|delay| format!("{delay}ULL"))
                    .collect::<Vec<_>>()
                    .join(", ");
                // A designated field closes every row, so the fields left out
                // are zero without a missing-initializer warning.
                let mut extra = format!(", .edge = {}", path.edge().code());
                match path.condition() {
                    IrPathCondition::None => {}
                    IrPathCondition::If(condition) => {
                        extra.push_str(&format!(", .condition = {}", condition + 1))
                    }
                    IrPathCondition::IfNone => extra.push_str(", .ifnone = 1"),
                }
                if path.group() != 0 {
                    extra.push_str(&format!(", .group = {}", path.group()));
                }
                if path.interconnect() {
                    extra.push_str(", .interconnect = 1");
                }
                if let Some(limits) = path.limits() {
                    let table = format!("{name}__limits_{dest_index}_{path_index}");
                    let values = limits
                        .reject()
                        .iter()
                        .chain(limits.error())
                        .map(|limit| format!("{limit}ULL"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    out.push_str(&format!(
                        "static uint64_t {table}[2 * LLG_PATH_TRANSITIONS] = {{ {values} }};\n"
                    ));
                    extra.push_str(&format!(", .limits = {table}"));
                }
                rows.push(format!(
                    "{{ {}, {}, {}, {}, {}, {}, {}, {{ {delays} }}, {}{extra} }}",
                    path.source,
                    path.source_lsb,
                    path.source_width,
                    path.dest_lsb,
                    path.dest_width,
                    u8::from(path.full),
                    path.polarity.code(),
                    crate::sim::emit_c::constants::c_string_literal(&path.origin),
                ));
            }
            out.push_str(&format!(
                "static llg_module_path_t {name}__paths_{dest_index}[] = {{ {} }};\n",
                rows.join(", ")
            ));
            let styles = match destination.styles() {
                Some(styles) => {
                    let table = format!("{name}__styles_{dest_index}");
                    out.push_str(&format!(
                        "static const uint8_t {table}[] = {{ {} }};\n",
                        styles
                            .iter()
                            .map(u8::to_string)
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    format!(", .styles = {table}")
                }
                None => ", .styles = NULL".to_owned(),
            };
            dests.push(format!(
                "{{ &{}, {name}__paths_{dest_index}, {}, NULL, NULL{styles} }}",
                model.net_group(destination.group()).c_name,
                destination.paths().len()
            ));
        }
        let conditions = if set.conditions().is_empty() {
            ", .conditions = NULL".to_owned()
        } else {
            out.push_str(&format!(
                "static void {name}__conditions(sv4_t* out, void* context);\n"
            ));
            format!(
                ", .conditions = {name}__conditions, .n_conditions = {}",
                set.conditions().len()
            )
        };
        out.push_str(&format!(
            "static llg_module_path_dest_t {name}__dests[] = {{ {} }};\n\
             static llg_module_path_set_t {name} = {{ {}, {sources}, {}, {name}__dests, {}, NULL, NULL, NULL{conditions} }};\n",
            dests.join(", "),
            crate::sim::emit_c::constants::c_string_literal(set.scope()),
            set.sources().len(),
            set.destinations().len(),
        ));
    }
}

/// Timing check tables: one `llg_timing_check_set_t` per instance with
/// checks, after the signals its terminals name. Limits and delays stay
/// writable for back-annotation; records use designated fields so the
/// runtime state fields start zero. Nothing is emitted for a design without
/// checks.
fn render_timing_checks(model: &IrModel, out: &mut String) {
    use crate::sim::emit_c::constants::c_string_literal;
    use crate::sim::ir::IrTimingLineStorage;
    let condition = |condition: Option<crate::sim::ir::IrTimingCondition>| {
        condition.map_or((0, false), |condition| {
            (condition.index + 1, condition.nondeterministic)
        })
    };
    let event = |event: crate::sim::ir::IrTimingEvent| {
        let (first, first_nondeterministic) = condition(event.condition);
        let (second, second_nondeterministic) = condition(event.condition2);
        let nondeterministic =
            u8::from(first_nondeterministic) | (u8::from(second_nondeterministic) << 1);
        let mut text = format!(
            "{{ .terminal = {}u, .edges = {}",
            event.terminal, event.edges
        );
        if nondeterministic != 0 {
            text.push_str(&format!(", .nondeterministic = {nondeterministic}"));
        }
        if first != 0 {
            text.push_str(&format!(", .condition = {first}u"));
        }
        if second != 0 {
            text.push_str(&format!(", .condition2 = {second}u"));
        }
        text.push_str(" }");
        text
    };
    for (index, set) in model.timing_checks().iter().enumerate() {
        let name = format!("llg_timing_set_{index}");
        let terminals = set
            .terminals()
            .iter()
            .map(|terminal| {
                let line = terminal
                    .line()
                    .map_or(String::new(), |line| format!(", .line = {}u", line + 1));
                format!(
                    "{{ .value = {}, .lsb = {}u, .n_bits = {}u{line} }}",
                    path_source_storage(model, terminal.signal()),
                    terminal.lsb(),
                    terminal.width()
                )
            })
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "static llg_timing_terminal_t {name}__terminals[] = {{ {} }};\n",
            terminals.join(", ")
        ));
        let checks = if set.checks().is_empty() {
            "NULL".to_owned()
        } else {
            let rows = set
                .checks()
                .iter()
                .map(|check| {
                    let (limit, limit2) = check.limits();
                    let flags = if check.flags() != 0 {
                        format!(".flags = {}, ", check.flags())
                    } else {
                        String::new()
                    };
                    format!(
                        "{{ .kind = {}, {flags}.reference = {}, .data = {}, .limit = {limit}LL, \
                         .limit2 = {limit2}LL, .notifier = {}u, .origin = {}, .text = {} }}",
                        check.kind().code(),
                        event(check.reference()),
                        event(check.data()),
                        check.notifier().map_or(0, |notifier| notifier + 1),
                        c_string_literal(check.origin()),
                        c_string_literal(check.text()),
                    )
                })
                .collect::<Vec<_>>();
            out.push_str(&format!(
                "static llg_timing_check_t {name}__checks[] = {{ {} }};\n",
                rows.join(", ")
            ));
            format!("{name}__checks")
        };
        let conditions = if set.conditions().is_empty() {
            "NULL".to_owned()
        } else {
            out.push_str(&format!(
                "static void {name}__conditions(sv4_t* out, void* context);\n"
            ));
            format!("{name}__conditions")
        };
        let notifiers = if set.notifiers().is_empty() {
            "NULL".to_owned()
        } else {
            let storage = set
                .notifiers()
                .iter()
                .map(|notifier| path_source_storage(model, *notifier))
                .collect::<Vec<_>>();
            out.push_str(&format!(
                "static sv4_t* const {name}__notifiers[] = {{ {} }};\n",
                storage.join(", ")
            ));
            format!("{name}__notifiers")
        };
        let lines = if set.lines().is_empty() {
            "NULL".to_owned()
        } else {
            let rows = set
                .lines()
                .iter()
                .map(|line| {
                    let storage = match line.storage() {
                        IrTimingLineStorage::Net { group, slot } => format!(
                            ".net = &{}, .slot = {slot}, ",
                            model.net_groups()[group].c_name
                        ),
                        IrTimingLineStorage::Variable => String::new(),
                    };
                    let (rise, fall) = line.delays();
                    format!(
                        "{{ .source = {}u, .target = {}u, {storage}.rise = {rise}ULL, \
                         .fall = {fall}ULL }}",
                        line.source(),
                        line.target()
                    )
                })
                .collect::<Vec<_>>();
            out.push_str(&format!(
                "static llg_timing_line_t {name}__lines[] = {{ {} }};\n",
                rows.join(", ")
            ));
            format!("{name}__lines")
        };
        // Pairs SDF annotation resettles (only in annotated designs).
        let pairs = if set.pairs().is_empty() {
            String::new()
        } else {
            let row = |row: Option<u32>| row.map_or(-1, i64::from);
            let rows = set
                .pairs()
                .iter()
                .map(|pair| {
                    format!(
                        "{{ .reference = {}u, .reference_edges = {}, .data = {}u, .data_edges = \
                         {}, .before = {}LL, .after = {}LL, .before_row = {}, .after_row = {}, \
                         .text = {}, .text_after = {}, .before_text = {}, .after_text = {} }}",
                        pair.reference,
                        pair.reference_edges,
                        pair.data,
                        pair.data_edges,
                        pair.before,
                        pair.after,
                        row(pair.before_row),
                        row(pair.after_row),
                        c_string_literal(&pair.text),
                        c_string_literal(&pair.text_after),
                        c_string_literal(&pair.before_text),
                        c_string_literal(&pair.after_text),
                    )
                })
                .collect::<Vec<_>>();
            out.push_str(&format!(
                "static llg_timing_pair_t {name}__pairs[] = {{ {} }};\n",
                rows.join(", ")
            ));
            format!(
                ", .pairs = {name}__pairs, .n_pairs = {}u",
                set.pairs().len()
            )
        };
        out.push_str(&format!(
            "static llg_timing_check_set_t {name} = {{ .scope = {}, .terminals = {name}__terminals, \
             .n_terminals = {}u, .checks = {checks}, .n_checks = {}u, .conditions = \
             {conditions}, .n_conditions = {}u, .notifiers = {notifiers}, .n_notifiers = {}u, \
             .lines = {lines}, .n_lines = {}u{pairs} }};\n",
            c_string_literal(set.scope()),
            set.terminals().len(),
            set.checks().len(),
            set.conditions().len(),
            set.notifiers().len(),
            set.lines().len(),
        ));
    }
}

/// SDF annotation tables (IEEE 1800-2009 32): the writable delays of the
/// annotated gates, and per annotation its values and operations, each
/// pointing at the path row, timing check or gate delays it changes.
/// Nothing is emitted for a design without `$sdf_annotate` and `--sdf`.
fn render_sdf_annotations(model: &IrModel, out: &mut String) {
    use crate::sim::ir::IrSdfTarget;
    let gates = model.sdf_gate_delays();
    if !gates.is_empty() {
        let values = gates
            .iter()
            .map(|delay| {
                format!(
                    "{}ULL, {}ULL, {}ULL",
                    delay.rise, delay.fall, delay.turn_off
                )
            })
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "static uint64_t llg_sdf_gate_delays[{}] = {{ {} }};\n",
            3 * gates.len(),
            values.join(", ")
        ));
    }
    // Each interconnect load keeps the delays of its sources; its path
    // takes their combination.
    for (index, load) in model.sdf_loads().iter().enumerate() {
        let name = format!("llg_sdf_load_{index}");
        out.push_str(&format!(
            "static uint64_t {name}__entries[{}];\nstatic uint8_t {name}__annotated[{}];\n\
             static llg_sdf_load_t {name} = {{ .path = &llg_path_set_{}__paths_{}[{}], \
             .entries = {name}__entries, .annotated = {name}__annotated, .n_entries = {}u }};\n",
            12 * load.entries,
            load.entries,
            load.set,
            load.destination,
            load.path,
            load.entries,
        ));
    }
    for (index, annotation) in model.sdf_annotations().iter().enumerate() {
        let name = format!("llg_sdf_annotation_{index}");
        let file = annotation
            .file
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || "._/-".contains(ch) {
                    ch
                } else {
                    '_'
                }
            })
            .collect::<String>();
        out.push_str(&format!("/* SDF file {file} */\n"));
        let values = if annotation.values.is_empty() {
            "NULL".to_owned()
        } else {
            let values = annotation
                .values
                .iter()
                .map(|value| {
                    if *value == i64::MIN {
                        "(-9223372036854775807LL - 1)".to_owned()
                    } else {
                        format!("{value}LL")
                    }
                })
                .collect::<Vec<_>>();
            out.push_str(&format!(
                "static const int64_t {name}__values[] = {{ {} }};\n",
                values.join(", ")
            ));
            format!("{name}__values")
        };
        let ops = if annotation.ops.is_empty() {
            "NULL".to_owned()
        } else {
            let rows = annotation
                .ops
                .iter()
                .map(|op| {
                    let (kind, flags, target) = match op.target {
                        IrSdfTarget::Path {
                            set,
                            destination,
                            path,
                        } => (
                            "LLG_SDF_PATH",
                            0,
                            format!(".path = &llg_path_set_{set}__paths_{destination}[{path}]"),
                        ),
                        IrSdfTarget::Check { set, check, second } => (
                            "LLG_SDF_CHECK",
                            if second { 4 } else { 0 },
                            format!(
                                ".check = &llg_timing_set_{set}__checks[{check}], .checks = \
                                 &llg_timing_set_{set}"
                            ),
                        ),
                        IrSdfTarget::Pair { set, pair, after } => (
                            "LLG_SDF_PAIR",
                            if after { 4 } else { 0 },
                            format!(".index = {pair}u, .checks = &llg_timing_set_{set}"),
                        ),
                        IrSdfTarget::Load { load, entry } => (
                            "LLG_SDF_LOAD",
                            0,
                            format!(".index = {entry}u, .load = &llg_sdf_load_{load}"),
                        ),
                        IrSdfTarget::Gate { slot } => (
                            "LLG_SDF_GATE",
                            0,
                            format!(".gate = &llg_sdf_gate_delays[{}]", 3 * slot),
                        ),
                    };
                    format!(
                        "{{ .kind = {kind}, .flags = {}u, .delay = 0x{:x}u, .reject = 0x{:x}u, \
                         .error = 0x{:x}u, .derive = 0x{:x}u, .first = {}u, {target} }}",
                        op.flags | flags,
                        op.delay,
                        op.reject,
                        op.error,
                        op.derive,
                        op.first
                    )
                })
                .collect::<Vec<_>>();
            out.push_str(&format!(
                "static const llg_sdf_op_t {name}__ops[] = {{ {} }};\n",
                rows.join(",\n    ")
            ));
            format!("{name}__ops")
        };
        out.push_str(&format!(
            "static const llg_sdf_annotation_t {name} = {{ {ops}, {}u, {values} }};\n",
            annotation.ops.len()
        ));
    }
}

/// The generated condition evaluator of every timing check set with
/// conditioned events: `out[k]` receives condition `k`, read in the
/// instance's scope when a step evaluates a conditioned event.
pub(super) fn render_timing_condition_evaluators<'a, 'm>(
    ctx: &'a RCtx<'m>,
    names: &'a crate::sim::emit_c::owned::StorageNames<'m>,
    out: &mut String,
) -> Result<(), String> {
    for (index, set) in ctx.model.timing_checks().iter().enumerate() {
        if set.conditions().is_empty() {
            continue;
        }
        let evaluator = crate::sim::ir::IrPreFn::MonEval {
            c_name: format!("llg_timing_set_{index}__conditions"),
            args: set.conditions().to_vec(),
            context: None,
            item: false,
            real_item: false,
        };
        out.push_str(&super::super::owned::model::pre_function(
            ctx, &evaluator, names,
        )?);
    }
    Ok(())
}

/// The generated condition evaluator of every module path set with
/// state-dependent paths: `out[k]` receives condition `k`, read in the
/// instance's scope when the runtime schedules a destination transition.
pub(super) fn render_path_condition_evaluators<'a, 'm>(
    ctx: &'a RCtx<'m>,
    names: &'a crate::sim::emit_c::owned::StorageNames<'m>,
    out: &mut String,
) -> Result<(), String> {
    for (index, set) in ctx.model.module_paths().iter().enumerate() {
        if set.conditions().is_empty() {
            continue;
        }
        let evaluator = crate::sim::ir::IrPreFn::MonEval {
            c_name: format!("llg_path_set_{index}__conditions"),
            args: set.conditions().to_vec(),
            context: None,
            item: false,
            real_item: false,
        };
        out.push_str(&super::super::owned::model::pre_function(
            ctx, &evaluator, names,
        )?);
    }
    Ok(())
}
