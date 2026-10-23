//! Per-instance tables of cell libraries as data: module path sets and
//! timing check sets are templates shared by every instance of one cell
//! definition plus per-instance references, laid out by the runtime at
//! start.
use super::*;
use crate::sim::emit_c::constants::c_string_literal;
use crate::sim::ir::IrPathCondition;

/// Interns template tables by their element type and content.
#[derive(Default)]
struct Templates {
    index: HashMap<String, usize>,
    rows: String,
}

impl Templates {
    /// Index of the template whose full text is `key`, emitting its tables
    /// through `emit` when it is new.
    fn intern(&mut self, key: String, emit: impl FnOnce(usize) -> String) -> usize {
        let next = self.index.len();
        *self.index.entry(key).or_insert_with(|| {
            self.rows.push_str(&emit(next));
            next
        })
    }
}

/// Module path sets (IEEE 1800-2009 30.4): one `llg_module_path_template_t`
/// per distinct set configuration, the source storages and destination nets
/// of every set in two reference tables, and one `{ set, scope, template,
/// condition context }` row per set. `llg_module_paths_layout` copies the
/// template rows and limits into the writable pools of `model/storage.rs`
/// before startup back-annotation. `evaluators` maps a set's condition
/// evaluator to its shared body and instance record.
pub(super) fn module_path_tables(
    model: &IrModel,
    evaluators: &HashMap<String, (String, String)>,
    tables: &mut StorageTables,
    initialize: &mut String,
) {
    let sets = model.module_paths();
    if sets.is_empty() {
        return;
    }
    let mut interned = InternedTables::default();
    let mut templates = Templates::default();
    let mut template_rows = String::new();
    let mut sources = Vec::new();
    let mut nets = Vec::new();
    let mut set_rows = String::new();
    for (index, set) in sets.iter().enumerate() {
        let mut rows = Vec::new();
        let mut limits = Vec::new();
        let mut dest_paths = Vec::new();
        let mut styles = Vec::new();
        for destination in set.destinations() {
            dest_paths.push(format!("{}u", destination.paths().len()));
            styles.push(match destination.styles() {
                Some(values) => interned.name(
                    &mut tables.source,
                    "llg_path_styles",
                    "uint8_t",
                    values
                        .iter()
                        .map(u8::to_string)
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                None => "NULL".to_owned(),
            });
            for path in destination.paths() {
                let delays = path
                    .delays()
                    .iter()
                    .map(|delay| format!("{delay}ULL"))
                    .collect::<Vec<_>>()
                    .join(", ");
                // A designated field closes every row, so the fields left
                // out are zero without a missing-initializer warning.
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
                limits.push(match path.limits() {
                    Some(values) => interned.name(
                        &mut tables.source,
                        "llg_path_limits",
                        "uint64_t",
                        values
                            .reject()
                            .iter()
                            .chain(values.error())
                            .map(|limit| format!("{limit}ULL"))
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    None => "NULL".to_owned(),
                });
                rows.push(format!(
                    "{{ {}, {}, {}, {}, {}, {}, {}, {{ {delays} }}, {}{extra} }}",
                    path.source,
                    path.source_lsb,
                    path.source_width,
                    path.dest_lsb,
                    path.dest_width,
                    u8::from(path.full),
                    path.polarity.code(),
                    c_string_literal(&path.origin),
                ));
            }
        }
        let evaluator = format!("llg_path_set_{index}__conditions");
        let (conditions, context) = if set.conditions().is_empty() {
            ("NULL".to_owned(), "NULL".to_owned())
        } else {
            evaluators
                .get(&evaluator)
                .cloned()
                .unwrap_or((evaluator, "NULL".to_owned()))
        };
        let styles = if styles.iter().all(|style| style == "NULL") {
            None
        } else {
            Some(styles.join(", "))
        };
        let limits = if limits.iter().all(|limit| limit == "NULL") {
            None
        } else {
            Some(limits.join(", "))
        };
        let key = format!(
            "{}|{}|{}|{:?}|{:?}|{conditions}|{}",
            set.sources().len(),
            dest_paths.join(", "),
            rows.join(", "),
            styles,
            limits,
            set.conditions().len()
        );
        let template = templates.intern(key, |next| {
            let name = format!("llg_path_template_{next}");
            let mut text = format!(
                "static const llg_module_path_t {name}_rows[] = {{ {} }};\n\
                 static const uint32_t {name}_dest_paths[] = {{ {} }};\n",
                rows.join(", "),
                dest_paths.join(", ")
            );
            let styles = match &styles {
                Some(styles) => {
                    text.push_str(&format!(
                        "static const uint8_t* const {name}_styles[] = {{ {styles} }};\n"
                    ));
                    format!("{name}_styles")
                }
                None => "NULL".to_owned(),
            };
            let limits = match &limits {
                Some(limits) => {
                    text.push_str(&format!(
                        "static const uint64_t* const {name}_limits[] = {{ {limits} }};\n"
                    ));
                    format!("{name}_limits")
                }
                None => "NULL".to_owned(),
            };
            template_rows.push_str(&format!(
                "    {{ {}u, {}u, {name}_dest_paths, {styles}, {name}_rows, {limits}, {conditions}, {}u }},\n",
                set.sources().len(),
                set.destinations().len(),
                set.conditions().len()
            ));
            text
        });
        sources.extend(
            set.sources()
                .iter()
                .map(|signal| crate::sim::emit_c::model::path_source_storage(model, *signal)),
        );
        nets.extend(
            set.destinations()
                .iter()
                .map(|destination| format!("&{}", model.net_group(destination.group()).c_name)),
        );
        set_rows.push_str(&format!(
            "    {{ &llg_path_set_{index}, {}, {template}u, {context} }},\n",
            c_string_literal(set.scope())
        ));
    }
    tables.source.push_str(&templates.rows);
    tables.source.push_str(&format!(
        "static const llg_module_path_template_t llg_path_templates[] = {{\n{template_rows}}};\n"
    ));
    let sources = references(tables, "sv4_t", "llg_path_source_refs", &sources);
    let nets = references(tables, "llg_net_t", "llg_path_dest_nets", &nets);
    tables.source.push_str(&format!(
        "static const struct {{ llg_module_path_set_t* set; const char* scope; uint32_t path_template; const void* context; }} llg_path_sets[] = {{\n{set_rows}}};\n"
    ));
    let limited = sets
        .iter()
        .flat_map(|set| set.destinations())
        .flat_map(|destination| destination.paths())
        .any(|path| path.limits().is_some());
    let limit_pool = if limited {
        "llg_path_limit_pool"
    } else {
        "NULL"
    };
    initialize.push_str(&format!(
        "    {{\n        llg_module_path_pools_t pools = {{ {sources}, {nets}, llg_path_dest_pool, llg_path_row_pool, {limit_pool} }};\n        {}\n            llg_module_paths_layout(llg_path_sets[_llg_n].set, llg_path_sets[_llg_n].scope, &llg_path_templates[llg_path_sets[_llg_n].path_template], llg_path_sets[_llg_n].context, &pools);\n    }}\n",
        table_loop("llg_path_sets")
    ));
}

/// Timing check sets (IEEE 1800-2009 31): one `llg_timing_template_t` per
/// distinct set configuration (terminal bit ranges, checks with their limits
/// and texts, delayed-signal lines), the terminal and notifier storages and
/// line nets of every set in two reference tables, and one row per set.
/// `llg_timing_checks_layout` copies the template into the writable pools
/// of `model/storage.rs` before startup back-annotation.
pub(super) fn timing_check_tables(
    model: &IrModel,
    evaluators: &HashMap<String, (String, String)>,
    tables: &mut StorageTables,
    initialize: &mut String,
) {
    use crate::sim::ir::{IrTimingCondition, IrTimingEvent, IrTimingLineStorage};
    let sets = model.timing_checks();
    if sets.is_empty() {
        return;
    }
    let condition = |condition: Option<IrTimingCondition>| {
        condition.map_or((0, false), |condition| {
            (condition.index + 1, condition.nondeterministic)
        })
    };
    let event = |event: IrTimingEvent| {
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
    let mut templates = Templates::default();
    let mut template_rows = String::new();
    let mut values = Vec::new();
    let mut line_nets = Vec::new();
    let mut set_rows = String::new();
    for (index, set) in sets.iter().enumerate() {
        let terminals = set
            .terminals()
            .iter()
            .map(|terminal| {
                let line = terminal
                    .line()
                    .map_or(String::new(), |line| format!(", .line = {}u", line + 1));
                format!(
                    "{{ .lsb = {}u, .n_bits = {}u{line} }}",
                    terminal.lsb(),
                    terminal.width()
                )
            })
            .collect::<Vec<_>>();
        let checks = set
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
        let lines = set
            .lines()
            .iter()
            .map(|line| {
                let (rise, fall) = line.delays();
                let slot = match line.storage() {
                    IrTimingLineStorage::Net { slot, .. } => format!(".slot = {slot}, "),
                    IrTimingLineStorage::Variable => String::new(),
                };
                format!(
                    "{{ .source = {}u, .target = {}u, {slot}.rise = {rise}ULL, .fall = {fall}ULL }}",
                    line.source(),
                    line.target()
                )
            })
            .collect::<Vec<_>>();
        let evaluator = format!("llg_timing_set_{index}__conditions");
        let (conditions, context) = if set.conditions().is_empty() {
            ("NULL".to_owned(), "NULL".to_owned())
        } else {
            evaluators
                .get(&evaluator)
                .cloned()
                .unwrap_or((evaluator, "NULL".to_owned()))
        };
        let key = format!(
            "{}|{}|{}|{conditions}|{}|{}",
            terminals.join(", "),
            checks.join(", "),
            lines.join(", "),
            set.conditions().len(),
            set.notifiers().len()
        );
        let template = templates.intern(key, |next| {
            let name = format!("llg_timing_template_{next}");
            let mut text = format!(
                "static const llg_timing_terminal_t {name}_terminals[] = {{ {} }};\n",
                if terminals.is_empty() {
                    "{ 0 }".to_owned()
                } else {
                    terminals.join(", ")
                }
            );
            let table = |text: &mut String, element: &str, suffix: &str, rows: &[String]| {
                if rows.is_empty() {
                    return "NULL".to_owned();
                }
                text.push_str(&format!(
                    "static const {element} {name}_{suffix}[] = {{ {} }};\n",
                    rows.join(", ")
                ));
                format!("{name}_{suffix}")
            };
            let checks_table = table(&mut text, "llg_timing_check_t", "checks", &checks);
            let lines_table = table(&mut text, "llg_timing_line_t", "lines", &lines);
            template_rows.push_str(&format!(
                "    {{ {}u, {name}_terminals, {}u, {checks_table}, {conditions}, {}u, {}u, {}u, {lines_table} }},\n",
                terminals.len(),
                checks.len(),
                set.conditions().len(),
                set.notifiers().len(),
                lines.len(),
            ));
            text
        });
        values.extend(
            set.terminals()
                .iter()
                .map(|terminal| terminal.signal())
                .chain(set.notifiers().iter().copied())
                .map(|signal| crate::sim::emit_c::model::path_source_storage(model, signal)),
        );
        line_nets.extend(set.lines().iter().map(|line| match line.storage() {
            IrTimingLineStorage::Net { group, .. } => {
                format!("&{}", model.net_groups()[group].c_name)
            }
            IrTimingLineStorage::Variable => "NULL".to_owned(),
        }));
        let pairs = if set.pairs().is_empty() {
            "NULL, 0u".to_owned()
        } else {
            format!("llg_timing_set_{index}__pairs, {}u", set.pairs().len())
        };
        set_rows.push_str(&format!(
            "    {{ &llg_timing_set_{index}, {}, {template}u, {context}, {pairs} }},\n",
            c_string_literal(set.scope())
        ));
    }
    tables.source.push_str(&templates.rows);
    tables.source.push_str(&format!(
        "static const llg_timing_template_t llg_timing_templates[] = {{\n{template_rows}}};\n"
    ));
    let values = references(tables, "sv4_t", "llg_timing_values", &values);
    let line_nets = references(tables, "llg_net_t", "llg_timing_line_nets", &line_nets);
    tables.source.push_str(&format!(
        "static const struct {{ llg_timing_check_set_t* set; const char* scope; uint32_t checks_template; const void* context; llg_timing_pair_t* pairs; uint32_t n_pairs; }} llg_timing_sets[] = {{\n{set_rows}}};\n"
    ));
    let pool = |name: &'static str, used: bool| if used { name } else { "NULL" };
    let terminals = pool(
        "llg_timing_terminal_pool",
        sets.iter().any(|set| !set.terminals().is_empty()),
    );
    let checks = pool(
        "llg_timing_check_pool",
        sets.iter().any(|set| !set.checks().is_empty()),
    );
    let lines = pool(
        "llg_timing_line_pool",
        sets.iter().any(|set| !set.lines().is_empty()),
    );
    initialize.push_str(&format!(
        "    {{\n        llg_timing_pools_t pools = {{ {values}, {line_nets}, {terminals}, {checks}, {lines} }};\n        {}\n            llg_timing_checks_layout(llg_timing_sets[_llg_n].set, llg_timing_sets[_llg_n].scope, &llg_timing_templates[llg_timing_sets[_llg_n].checks_template], llg_timing_sets[_llg_n].context, llg_timing_sets[_llg_n].pairs, llg_timing_sets[_llg_n].n_pairs, &pools);\n    }}\n",
        table_loop("llg_timing_sets")
    ));
}

/// A constant pointer table of references, eight per line, or `NULL` when
/// there are none.
fn references(tables: &mut StorageTables, element: &str, name: &str, values: &[String]) -> String {
    if values.is_empty() {
        return "NULL".to_owned();
    }
    tables
        .source
        .push_str(&format!("static {element}* const {name}[] = {{\n"));
    for chunk in values.chunks(8) {
        tables
            .source
            .push_str(&format!("    {},\n", chunk.join(", ")));
    }
    tables.source.push_str("};\n");
    name.to_owned()
}
