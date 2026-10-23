//! Storage.

use super::*;

/// Net records declared by one C declaration; only the text width changes.
const NET_DECLARATIONS_PER_LINE: usize = 8;

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
    // Net records are zero until `llg_net_layout` configures them at start
    // from their shape (`initialization.rs`), so a net costs one name here.
    let mut groups_emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let names = model
        .net_groups
        .iter()
        .filter(|group| groups_emitted.insert(group.c_name.as_str()))
        .map(|group| group.c_name.as_str())
        .collect::<Vec<_>>();
    for chunk in names.chunks(NET_DECLARATIONS_PER_LINE) {
        out.push_str(&format!("static llg_net_t {};\n", chunk.join(", ")));
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
pub(in crate::sim::emit_c) fn path_source_storage(model: &IrModel, index: usize) -> String {
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

/// First pool row of every destination of every module path set: rows are
/// numbered in set, destination and path order (`llg_path_row_pool`), as
/// `llg_module_paths_layout` consumes them.
pub(in crate::sim::emit_c) fn path_row_starts(model: &IrModel) -> Vec<Vec<usize>> {
    let mut next = 0;
    model
        .module_paths()
        .iter()
        .map(|set| {
            set.destinations()
                .iter()
                .map(|destination| {
                    let first = next;
                    next += destination.paths().len();
                    first
                })
                .collect()
        })
        .collect()
}

/// Module path set records and the writable pools their rows, destinations
/// and limits live in. The records are zero until `llg_module_paths_layout`
/// configures them at start from their templates
/// (`owned/model/initialization`); back-annotation addresses rows in
/// `llg_path_row_pool`.
fn render_module_paths(model: &IrModel, out: &mut String) {
    let sets = model.module_paths();
    if sets.is_empty() {
        return;
    }
    let names = (0..sets.len())
        .map(|index| format!("llg_path_set_{index}"))
        .collect::<Vec<_>>();
    for chunk in names.chunks(NET_DECLARATIONS_PER_LINE) {
        out.push_str(&format!(
            "static llg_module_path_set_t {};\n",
            chunk.join(", ")
        ));
    }
    let destinations = sets
        .iter()
        .map(|set| set.destinations().len())
        .sum::<usize>();
    let rows = sets
        .iter()
        .flat_map(|set| set.destinations())
        .map(|destination| destination.paths().len())
        .sum::<usize>();
    let limited = sets
        .iter()
        .flat_map(|set| set.destinations())
        .flat_map(|destination| destination.paths())
        .filter(|path| path.limits().is_some())
        .count();
    out.push_str(&format!(
        "static llg_module_path_dest_t llg_path_dest_pool[{}];\nstatic llg_module_path_t llg_path_row_pool[{}];\n",
        destinations.max(1),
        rows.max(1)
    ));
    if limited > 0 {
        out.push_str(&format!(
            "static uint64_t llg_path_limit_pool[{}];\n",
            2 * 12 * limited
        ));
    }
}

/// First pool check of every timing check set (`llg_timing_check_pool`), in
/// set order, as `llg_timing_checks_layout` consumes them.
pub(in crate::sim::emit_c) fn timing_check_starts(model: &IrModel) -> Vec<usize> {
    let mut next = 0;
    model
        .timing_checks()
        .iter()
        .map(|set| {
            let first = next;
            next += set.checks().len();
            first
        })
        .collect()
}

/// Timing check set records, the writable pools of their terminals, checks
/// and delayed-signal lines, and the pair tables of sets whose combined
/// checks SDF annotation resettles. The records are zero until
/// `llg_timing_checks_layout` configures them at start from their templates
/// (`owned/model/initialization`); back-annotation addresses checks in
/// `llg_timing_check_pool`. Nothing is emitted for a design without checks.
fn render_timing_checks(model: &IrModel, out: &mut String) {
    use crate::sim::emit_c::constants::c_string_literal;
    let sets = model.timing_checks();
    if sets.is_empty() {
        return;
    }
    let names = (0..sets.len())
        .map(|index| format!("llg_timing_set_{index}"))
        .collect::<Vec<_>>();
    for chunk in names.chunks(NET_DECLARATIONS_PER_LINE) {
        out.push_str(&format!(
            "static llg_timing_check_set_t {};\n",
            chunk.join(", ")
        ));
    }
    let count = |items: fn(&crate::sim::ir::IrTimingCheckSet) -> usize| {
        sets.iter().map(items).sum::<usize>()
    };
    for (element, name, total) in [
        (
            "llg_timing_terminal_t",
            "llg_timing_terminal_pool",
            count(|set| set.terminals().len()),
        ),
        (
            "llg_timing_check_t",
            "llg_timing_check_pool",
            count(|set| set.checks().len()),
        ),
        (
            "llg_timing_line_t",
            "llg_timing_line_pool",
            count(|set| set.lines().len()),
        ),
    ] {
        if total > 0 {
            out.push_str(&format!("static {element} {name}[{total}];\n"));
        }
    }
    // Pairs SDF annotation resettles (only in annotated designs).
    for (index, set) in sets.iter().enumerate() {
        if set.pairs().is_empty() {
            continue;
        }
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
            "static llg_timing_pair_t llg_timing_set_{index}__pairs[] = {{ {} }};\n",
            rows.join(", ")
        ));
    }
}

/// SDF annotation tables (IEEE 1800-2009 32): the writable delays of the
/// annotated gates, and per annotation its values and operations, each
/// pointing at the path row, timing check or gate delays it changes.
/// Nothing is emitted for a design without `$sdf_annotate` and `--sdf`.
fn render_sdf_annotations(model: &IrModel, out: &mut String) {
    use crate::sim::ir::IrSdfTarget;
    let row_starts = path_row_starts(model);
    let check_starts = timing_check_starts(model);
    // A row outside the pool (an unvalidated target) stays a C error.
    let row = |set: u32, destination: u32, path: u32| {
        row_starts
            .get(set as usize)
            .and_then(|set| set.get(destination as usize))
            .map_or(usize::MAX, |first| first + path as usize)
    };
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
             static llg_sdf_load_t {name} = {{ .path = &llg_path_row_pool[{}], \
             .entries = {name}__entries, .annotated = {name}__annotated, .n_entries = {}u }};\n",
            12 * load.entries,
            load.entries,
            row(load.set, load.destination, load.path),
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
                            format!(
                                ".path = &llg_path_row_pool[{}]",
                                row(set, destination, path)
                            ),
                        ),
                        IrSdfTarget::Check { set, check, second } => (
                            "LLG_SDF_CHECK",
                            if second { 4 } else { 0 },
                            format!(
                                ".check = &llg_timing_check_pool[{}], .checks = \
                                 &llg_timing_set_{set}",
                                check_starts
                                    .get(set as usize)
                                    .map_or(usize::MAX, |first| first + check as usize)
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

/// The generated condition evaluators of module path sets with
/// state-dependent paths (`llg_path_set_<n>__conditions`) and of timing check
/// sets with conditions (`llg_timing_set_<n>__conditions`), by name: `out[k]`
/// receives condition `k`, read in the instance's scope. They are rendered
/// before instance sharing, which replaces the evaluators of one cell
/// definition by one body reading its instance record through `context`.
pub(super) fn condition_evaluator_sources<'a, 'm>(
    ctx: &'a RCtx<'m>,
    names: &'a crate::sim::emit_c::owned::StorageNames<'m>,
) -> Result<BTreeMap<String, String>, String> {
    let paths = ctx
        .model
        .module_paths()
        .iter()
        .enumerate()
        .map(|(index, set)| {
            (
                format!("llg_path_set_{index}__conditions"),
                set.conditions(),
            )
        });
    let checks = ctx
        .model
        .timing_checks()
        .iter()
        .enumerate()
        .map(|(index, set)| {
            (
                format!("llg_timing_set_{index}__conditions"),
                set.conditions(),
            )
        });
    let mut sources = BTreeMap::new();
    for (c_name, conditions) in paths.chain(checks) {
        if conditions.is_empty() {
            continue;
        }
        let evaluator = crate::sim::ir::IrPreFn::MonEval {
            c_name: c_name.clone(),
            args: conditions.to_vec(),
            context: None,
            item: false,
            real_item: false,
        };
        let source = super::super::owned::model::pre_function(ctx, &evaluator, names)?;
        sources.insert(c_name, super::retained(source));
    }
    Ok(sources)
}
