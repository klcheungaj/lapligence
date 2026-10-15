//! Functional coverage (IEEE 1800-2009 19): static coverpoint tables and
//! the statements and queries that drive `coverage/coverage.c`.
use super::native::NativeKind;
use super::*;
use crate::sim::ir::{
    IrCoverAutomatic, IrCoverBinShape, IrCoverBound, IrCoverCount, IrCoverCross,
    IrCoverTransitions, IrCoverageApiCall, IrCoverageQuery, IrCoverageScope, IrCoverageStmt,
    IrCoverageTextQuery, IrPlusArgTarget,
};

fn group_name(group: usize) -> String {
    format!("llg_cov_group_{group}")
}

fn type_name(coverage_type: usize) -> String {
    format!("llg_cov_type_{coverage_type}")
}

fn bound(bound: IrCoverBound) -> (&'static str, u64) {
    match bound {
        IrCoverBound::Key(key) => ("LLG_COV_BOUND_KEY", key),
        IrCoverBound::Slot(slot) => ("LLG_COV_BOUND_SLOT", u64::from(slot)),
        IrCoverBound::Min => ("LLG_COV_BOUND_MIN", 0),
        IrCoverBound::Max => ("LLG_COV_BOUND_MAX", 0),
    }
}

fn optional_string(value: Option<&str>) -> String {
    value.map_or_else(|| "NULL".to_owned(), c_string_literal)
}

/// A comment option: `NULL` stands for the default "".
fn comment(value: &str) -> String {
    optional_string(Some(value).filter(|value| !value.is_empty()))
}

/// A static array named `name`, or `NULL` when it has no rows.
fn static_array(out: &mut String, element: &str, name: &str, rows: Vec<String>) -> String {
    if rows.is_empty() {
        return "NULL".to_owned();
    }
    out.push_str(&format!(
        "static const {element} {name}[] = {{\n    {}\n}};\n",
        rows.join(",\n    ")
    ));
    name.to_owned()
}

fn item_rows(items: &[crate::sim::ir::IrCoverItem]) -> Vec<String> {
    items
        .iter()
        .map(|item| {
            let (lo_kind, lo) = bound(item.lo);
            let (hi_kind, hi) = bound(item.hi);
            format!(
                "{{ UINT64_C({lo}), UINT64_C({hi}), {lo_kind}, {hi_kind}, {} }}",
                optional_string(item.site.as_deref())
            )
        })
        .collect()
}

/// The tables of one cross; returns the row of the group's cross array.
fn cross_tables(out: &mut String, prefix: &str, cross: &IrCoverCross) -> String {
    let dims = static_array(
        out,
        "uint32_t",
        &format!("{prefix}_dims"),
        cross
            .points
            .iter()
            .map(|point| format!("{point}u"))
            .collect(),
    );
    let items = static_array(
        out,
        "llg_cov_item_t",
        &format!("{prefix}_items"),
        item_rows(&cross.items),
    );
    let selects = static_array(
        out,
        "llg_cov_select_t",
        &format!("{prefix}_selects"),
        cross
            .selects
            .iter()
            .map(|select| {
                format!(
                    "{{ .dim = {}u, .decl = {}, .intersect = {}u, .negate = {}u, .first_item = {}u, .n_items = {}u }}",
                    select.dim,
                    select.decl.map_or(-1, i64::from),
                    u8::from(select.intersect),
                    u8::from(select.negate),
                    select.first_item,
                    select.n_items
                )
            })
            .collect(),
    );
    let mut boxes = Vec::new();
    let mut box_selects = Vec::new();
    let mut bins = Vec::with_capacity(cross.bins.len());
    for bin in &cross.bins {
        let first_box = boxes.len();
        for conjunction in &bin.boxes {
            boxes.push(format!(
                "{{ .first_select = {}u, .n_selects = {}u }}",
                box_selects.len(),
                conjunction.len()
            ));
            box_selects.extend(conjunction.iter().map(|select| format!("{select}u")));
        }
        bins.push(format!(
            "{{ .name = {}, .site = {}, .kind = {}u, .guard = {}, .first_box = {first_box}u, .n_boxes = {}u }}",
            c_string_literal(&bin.name),
            c_string_literal(&bin.site),
            bin.kind.code(),
            bin.guard.map_or(-1, i64::from),
            bin.boxes.len()
        ));
    }
    let n_boxes = boxes.len();
    let boxes = static_array(out, "llg_cov_box_t", &format!("{prefix}_boxes"), boxes);
    let box_selects = static_array(
        out,
        "uint32_t",
        &format!("{prefix}_box_selects"),
        box_selects,
    );
    let bins = static_array(out, "llg_cov_xbin_t", &format!("{prefix}_xbins"), bins);
    format!(
        "{{ .name = {}, .n_dims = {}u, .dims = {dims}, .n_selects = {}u, .selects = {selects}, .items = {items}, .n_boxes = {n_boxes}u, .boxes = {boxes}, .box_selects = {box_selects}, .n_bins = {}u, .bins = {bins}, .dynamic = {}u }}",
        c_string_literal(&cross.name),
        cross.points.len(),
        cross.selects.len(),
        cross.bins.len(),
        u8::from(cross.dynamic()),
    )
}

/// The transition tables of one coverpoint; returns the table's name.
fn transition_tables(out: &mut String, prefix: &str, tables: &IrCoverTransitions) -> String {
    let u64_rows = |values: &[u64]| {
        values
            .iter()
            .map(|value| format!("UINT64_C({value})"))
            .collect::<Vec<_>>()
    };
    let u32_rows = |values: &[u32]| {
        values
            .iter()
            .map(|value| format!("{value}u"))
            .collect::<Vec<_>>()
    };
    let bounds = static_array(
        out,
        "uint64_t",
        &format!("{prefix}_tbounds"),
        u64_rows(&tables.bounds),
    );
    let accept = static_array(
        out,
        "uint64_t",
        &format!("{prefix}_taccept"),
        u64_rows(&tables.accept),
    );
    let steps = static_array(
        out,
        "llg_cov_step_t",
        &format!("{prefix}_tsteps"),
        tables
            .steps
            .iter()
            .map(|step| {
                format!(
                    "{{ .repeat = {}u, .last = {}u, .lo = {}u, .hi = {}u, .state = {}u, .set = {}u }}",
                    step.repeat.code(),
                    u8::from(step.last),
                    step.lo,
                    step.hi,
                    step.state,
                    step.set
                )
            })
            .collect(),
    );
    let sets = static_array(
        out,
        "llg_cov_tset_t",
        &format!("{prefix}_tsets"),
        tables
            .sets
            .iter()
            .map(|set| {
                format!(
                    "{{ .bin = {}u, .first_step = {}u, .n_steps = {}u }}",
                    set.bin, set.first_step, set.steps
                )
            })
            .collect(),
    );
    let start_index = static_array(
        out,
        "uint32_t",
        &format!("{prefix}_tstart_index"),
        u32_rows(&tables.start_index),
    );
    let start_sets = static_array(
        out,
        "uint32_t",
        &format!("{prefix}_tstart_sets"),
        u32_rows(&tables.start_sets),
    );
    let bins = static_array(
        out,
        "llg_cov_tbin_t",
        &format!("{prefix}_tbins"),
        tables
            .bins
            .iter()
            .map(|bin| {
                format!(
                    "{{ .name = {}, .site = {}, .kind = {}u, .array = {}u, .guard = {}, .coverable = UINT64_C({}) }}",
                    c_string_literal(&bin.name),
                    c_string_literal(&bin.site),
                    bin.kind.code(),
                    u8::from(bin.array),
                    bin.guard.map_or(-1, i64::from),
                    bin.coverable
                )
            })
            .collect(),
    );
    let overlaps = static_array(
        out,
        "uint32_t",
        &format!("{prefix}_toverlaps"),
        tables
            .overlaps
            .iter()
            .flat_map(|(a, b)| [format!("{a}u"), format!("{b}u")])
            .collect(),
    );
    let name = format!("{prefix}_trans");
    out.push_str(&format!(
        "static const llg_cov_trans_t {name} = {{ .n_classes = {}u, .bounds = {bounds}, .words = {}u, .accept = {accept}, .n_steps = {}u, .steps = {steps}, .n_sets = {}u, .sets = {sets}, .start_index = {start_index}, .start_sets = {start_sets}, .n_states = {}u, .window = {}u, .n_bins = {}u, .bins = {bins}, .default_bin = {}, .n_overlaps = {}u, .overlaps = {overlaps} }};\n",
        tables.bounds.len(),
        tables.words,
        tables.steps.len(),
        tables.sets.len(),
        tables.states,
        tables.window,
        tables.bins.len(),
        tables.default_bin.map_or(-1, i64::from),
        tables.overlaps.len()
    ));
    format!("&{name}")
}

/// Static metadata of every covergroup; nothing for a model without one.
pub(in crate::sim::emit_c) fn tables(model: &IrModel, out: &mut String) {
    for (index, coverage_type) in model.coverage_types.iter().enumerate() {
        let name = type_name(index);
        let items = static_array(
            out,
            "llg_cov_type_decl_t",
            &format!("{name}_items"),
            coverage_type
                .items
                .iter()
                .map(|item| {
                    format!(
                        "{{ INT64_C({}), INT64_C({}), {} }}",
                        item.weight,
                        item.goal,
                        comment(&item.comment)
                    )
                })
                .collect(),
        );
        // Current values start at the declared ones (19.7.1).
        out.push_str(&format!(
            "static LLG_MAYBE_UNUSED llg_cov_type_t {name} = {{ .name = {}, .weight = INT64_C({weight}), .merge = {merge}u, .strobe = {}u, .declared_weight = INT64_C({weight}), .declared_merge = {merge}u, .goal = INT64_C({goal}), .declared_goal = INT64_C({goal}), .declared_comment = {}, .n_items = {}u, .declared_items = {items} }};\n",
            c_string_literal(&coverage_type.name),
            u8::from(coverage_type.strobe),
            comment(&coverage_type.comment),
            coverage_type.items.len(),
            weight = coverage_type.weight,
            merge = u8::from(coverage_type.merge),
            goal = coverage_type.goal,
        ));
    }
    for (index, group) in model.covergroups.iter().enumerate() {
        let prefix = group_name(index);
        let mut points = Vec::with_capacity(group.points.len());
        for (point_index, point) in group.points.iter().enumerate() {
            let point_prefix = format!("{prefix}_{point_index}");
            let (kind, keys) = match &point.automatic {
                IrCoverAutomatic::Auto => ("LLG_COV_POINT_AUTO", None),
                IrCoverAutomatic::Enum(keys) => ("LLG_COV_POINT_ENUM", Some(keys)),
                IrCoverAutomatic::None => ("LLG_COV_POINT_EXPLICIT", None),
            };
            let keys_name = match keys {
                Some(keys) => {
                    out.push_str(&format!(
                        "static const uint64_t {point_prefix}_keys[] = {{ {} }};\n",
                        keys.iter()
                            .map(|key| format!("UINT64_C({key})"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ));
                    format!("{point_prefix}_keys")
                }
                None => "NULL".to_owned(),
            };
            let items_name = static_array(
                out,
                "llg_cov_item_t",
                &format!("{point_prefix}_items"),
                item_rows(&point.items),
            );
            let bins_name = if point.bins.is_empty() {
                "NULL".to_owned()
            } else {
                let rows = point
                    .bins
                    .iter()
                    .map(|bin| {
                        let (shape, count_is_slot, count) = match bin.shape {
                            IrCoverBinShape::Scalar => ("LLG_COV_SHAPE_SCALAR", 0, 0),
                            IrCoverBinShape::PerValue => ("LLG_COV_SHAPE_PER_VALUE", 0, 0),
                            IrCoverBinShape::Count(IrCoverCount::Const(count)) => {
                                ("LLG_COV_SHAPE_COUNT", 0, count)
                            }
                            IrCoverBinShape::Count(IrCoverCount::Slot(slot)) => {
                                ("LLG_COV_SHAPE_COUNT", 1, u64::from(slot))
                            }
                        };
                        format!(
                            "{{ .name = {}, .site = {}, .kind = {}u, .shape = {shape}, .count_is_slot = {count_is_slot}u, .guard = {}, .count = UINT64_C({count}), .first_item = {}u, .n_items = {}u }}",
                            c_string_literal(&bin.name),
                            c_string_literal(&bin.site),
                            bin.kind.code(),
                            bin.guard.map_or(-1, i64::from),
                            bin.first_item,
                            bin.n_items
                        )
                    })
                    .collect::<Vec<_>>();
                out.push_str(&format!(
                    "static const llg_cov_bin_t {point_prefix}_bins[] = {{\n    {}\n}};\n",
                    rows.join(",\n    ")
                ));
                format!("{point_prefix}_bins")
            };
            let trans = point.transitions.as_ref().map_or_else(
                || "NULL".to_owned(),
                |tables| transition_tables(out, &point_prefix, tables),
            );
            points.push(format!(
                "{{ .name = {}, .width = {}u, .is_signed = {}u, .kind = {kind}, .dynamic = {}u, .n_keys = {}u, .keys = {keys_name}, .n_bins = {}u, .bins = {bins_name}, .items = {items_name}, .trans = {trans}, .crossed = {}u }}",
                c_string_literal(&point.name),
                point.width,
                u8::from(point.signed),
                u8::from(point.dynamic()),
                keys.map_or(0, Vec::len),
                point.bins.len(),
                u8::from(point.crossed),
            ));
        }
        let crosses = group
            .crosses
            .iter()
            .enumerate()
            .map(|(index, cross)| cross_tables(out, &format!("{prefix}_x{index}"), cross))
            .collect::<Vec<_>>();
        let crosses_name = static_array(
            out,
            "llg_cov_cross_t",
            &format!("{prefix}_crosses"),
            crosses,
        );
        let points_name = if points.is_empty() {
            "NULL".to_owned()
        } else {
            out.push_str(&format!(
                "static const llg_cov_point_t {prefix}_points[] = {{\n    {}\n}};\n",
                points.join(",\n    ")
            ));
            format!("{prefix}_points")
        };
        let strobe = match group.strobe {
            None => "NULL".to_owned(),
            Some(function) => {
                // The Postponed flush calls the formal-free sample method
                // with only its receiver, at recursion depth zero.
                let sample = &model.funcs[function].c_name;
                let trampoline = format!("{prefix}_strobe");
                out.push_str(&format!(
                    "static void {sample}(void * _this, int depth);\nstatic void {trampoline}(void* object) {{ {sample}(object, 0); }}\n"
                ));
                trampoline
            }
        };
        out.push_str(&format!(
            "// Unreferenced when every handle of the group stays null.\nstatic LLG_MAYBE_UNUSED llg_cov_group_t {prefix} = {{ .name = {}, .path = {}, .n_points = {}u, .points = {points_name}, .n_crosses = {}u, .crosses = {crosses_name}, .n_slots = {}u, .type = &{}, .strobe_sample = {strobe} }};\n",
            c_string_literal(&group.name),
            c_string_literal(&group.path),
            group.points.len(),
            group.crosses.len(),
            group.slots,
            type_name(group.coverage_type)
        ));
    }
    if model.coverage_types.is_empty() {
        return;
    }
    // The database schema and the option reset of llg_rt_cleanup (19.9).
    let groups = (0..model.covergroups.len())
        .map(|index| format!("&{}", group_name(index)))
        .collect::<Vec<_>>();
    let types = (0..model.coverage_types.len())
        .map(|index| format!("&{}", type_name(index)))
        .collect::<Vec<_>>();
    if !groups.is_empty() {
        out.push_str(&format!(
            "static llg_cov_group_t* const llg_model_cov_groups[] = {{ {} }};\n",
            groups.join(", ")
        ));
    }
    out.push_str(&format!(
        "static llg_cov_type_t* const llg_model_cov_types[] = {{ {} }};\n",
        types.join(", ")
    ));
}

/// The assertion coverage item and instance tables (IEEE 1800-2009
/// 40.2.2, 40.3.2), present only in models that keep them. Items carry
/// their runtime counters, zeroed by `llg_acov_model`.
pub(in crate::sim::emit_c) fn assertion_tables(model: &IrModel, out: &mut String) {
    let Some(table) = &model.assertion_coverage else {
        return;
    };
    let index = |instance: Option<u32>| instance.map_or(-1, i64::from);
    if !table.items.is_empty() {
        let rows = table
            .items
            .iter()
            .map(|item| {
                format!(
                    "{{ .identity = UINT64_C({}), .name = {}, .full_name = {}, .instance = {}, .kind = {}, .form = {} }}",
                    item.identity,
                    c_string_literal(&item.name),
                    c_string_literal(&item.full_name),
                    index(item.instance),
                    item.kind.runtime_name(),
                    item.form.runtime_name()
                )
            })
            .collect::<Vec<_>>();
        out.push_str(&format!(
            "static llg_acov_item_t llg_model_acov_items[] = {{\n    {}\n}};\n",
            rows.join(",\n    ")
        ));
    }
    let rows = table
        .instances
        .iter()
        .map(|instance| {
            format!(
                "{{ .path = {}, .definition = {}, .parent = {} }}",
                c_string_literal(&instance.path),
                c_string_literal(&instance.definition),
                index(instance.parent)
            )
        })
        .collect::<Vec<_>>();
    static_array(out, "llg_acov_instance_t", "llg_model_acov_instances", rows);
}

/// The startup calls that register the model's coverage declarations and
/// assertion coverage tables; empty for a model without either.
pub(in crate::sim::emit_c) fn startup(model: &IrModel) -> String {
    let mut out = String::new();
    if !model.coverage_types.is_empty() {
        let groups = if model.covergroups.is_empty() {
            "NULL"
        } else {
            "llg_model_cov_groups"
        };
        out.push_str(&format!(
            "    llg_cov_model({groups}, {}u, llg_model_cov_types, {}u);\n",
            model.covergroups.len(),
            model.coverage_types.len()
        ));
    }
    if let Some(table) = &model.assertion_coverage {
        // Before the assertions register: each binds its item by identity.
        out.push_str(&format!(
            "    llg_acov_model({}, {}u, {}, {}u);\n",
            if table.items.is_empty() {
                "NULL"
            } else {
                "llg_model_acov_items"
            },
            table.items.len(),
            if table.instances.is_empty() {
                "NULL"
            } else {
                "llg_model_acov_instances"
            },
            table.instances.len()
        ));
    }
    out
}

fn point_index(point: Option<u32>) -> String {
    point.map_or_else(|| "-1".to_owned(), |point| format!("{point}"))
}

impl Frame<'_, '_> {
    pub(super) fn coverage_statement(&mut self, operation: &IrCoverageStmt) -> Result<(), String> {
        match operation {
            IrCoverageStmt::Create { group, target } => {
                let address = self.native_lookup(target, NativeKind::Chandle)?.address;
                self.line(format!(
                    "*({address}) = llg_cov_new(&{});",
                    group_name(*group)
                ));
            }
            IrCoverageStmt::Option {
                record,
                point,
                option,
                value,
                site,
            } => {
                let record = self.chandle(record)?;
                let record = self.scalar("void*", record);
                let value = self.expression(value)?;
                self.line(format!(
                    "llg_cov_option({record}, {}, {}u, {}, {});",
                    point_index(*point),
                    option.code(),
                    value.code,
                    c_string_literal(site)
                ));
                self.discard(value);
            }
            IrCoverageStmt::Text {
                record,
                point,
                which,
                value,
            } => {
                let record = self.chandle(record)?;
                let record = self.scalar("void*", record);
                let value = self.string(value)?;
                self.line(format!(
                    "llg_cov_text({record}, {}, {}u, {});",
                    point_index(*point),
                    which.code(),
                    value.take_string()
                ));
                self.native_discard(value);
            }
            IrCoverageStmt::TypeOption {
                coverage_type,
                point,
                option,
                value,
                site,
            } => {
                let value = self.expression(value)?;
                self.line(format!(
                    "llg_cov_type_option(&{}, {}, {}u, {}, {});",
                    type_name(*coverage_type),
                    point_index(*point),
                    option.code(),
                    value.code,
                    c_string_literal(site)
                ));
                self.discard(value);
            }
            IrCoverageStmt::TypeText {
                coverage_type,
                point,
                value,
            } => {
                let value = self.string(value)?;
                self.line(format!(
                    "llg_cov_type_text(&{}, {}, {});",
                    type_name(*coverage_type),
                    point_index(*point),
                    value.take_string()
                ));
                self.native_discard(value);
            }
            IrCoverageStmt::SetDbName { name } => {
                let name = self.string(name)?;
                self.line(format!("llg_cov_set_db_name({});", name.take_string()));
                self.native_discard(name);
            }
            IrCoverageStmt::LoadDb { name, site } => {
                let name = self.string(name)?;
                self.line(format!(
                    "llg_cov_load_db({}, {});",
                    name.take_string(),
                    c_string_literal(site)
                ));
                self.native_discard(name);
            }
            IrCoverageStmt::Slot {
                record,
                slot,
                value,
            } => {
                let record = self.chandle(record)?;
                let record = self.scalar("void*", record);
                let value = self.expression(value)?;
                self.line(format!("llg_cov_slot({record}, {slot}u, {});", value.code));
                self.discard(value);
            }
            IrCoverageStmt::Ready { record } => {
                let record = self.chandle(record)?;
                self.line(format!("llg_cov_ready({record});"));
            }
            IrCoverageStmt::Register { group, object } => {
                let object = self.chandle(object)?;
                self.line(format!(
                    "llg_cov_register(&{}, {object});",
                    group_name(*group)
                ));
            }
            IrCoverageStmt::Sample {
                record,
                guards,
                points,
                crosses,
            } => {
                let record = self.chandle(record)?;
                let record = self.scalar("void*", record);
                // Bin guards are evaluated once per sample, before any value
                // is counted (19.5).
                let guard_array = if guards.is_empty() {
                    "NULL".to_owned()
                } else {
                    let array = self.declare_array("uint8_t", "cov_guards", guards.len());
                    for (index, guard) in guards.iter().enumerate() {
                        let condition = self.condition(guard)?;
                        self.line(format!("{array}[{index}] = (uint8_t)({condition});"));
                    }
                    array
                };
                if !crosses.is_empty() {
                    // Crosses read the bins this sample's coverpoints count.
                    self.line(format!("llg_cov_sample_begin({record});"));
                }
                for sample in points {
                    // The value is evaluated only when its guard holds.
                    let guarded = match &sample.iff {
                        Some(iff) => {
                            let condition = self.condition(iff)?;
                            self.line(format!("if ({condition}) {{"));
                            true
                        }
                        None => false,
                    };
                    // A plain read is passed by address without a copy;
                    // the borrow ends with the call.
                    let value = self.operand(&sample.value)?;
                    self.line(format!(
                        "llg_cov_hit({record}, {}u, {}, {guard_array});",
                        sample.point,
                        value.address()
                    ));
                    self.discard(value);
                    if guarded && sample.history {
                        // A skipped sample ends the sequences in progress
                        // (decision S45-D9).
                        self.line("} else {");
                        self.line(format!("llg_cov_skip({record}, {}u);", sample.point));
                        self.line("}");
                    } else if guarded {
                        self.line("}");
                    }
                }
                for cross in crosses {
                    let guarded = match &cross.iff {
                        Some(iff) => {
                            let condition = self.condition(iff)?;
                            self.line(format!("if ({condition}) {{"));
                            true
                        }
                        None => false,
                    };
                    self.line(format!(
                        "llg_cov_cross_hit({record}, {}u, {guard_array});",
                        cross.cross
                    ));
                    if guarded {
                        self.line("}");
                    }
                }
            }
            IrCoverageStmt::Control {
                record,
                point,
                start,
            } => {
                let record = self.chandle(record)?;
                self.line(format!(
                    "llg_cov_control({record}, {}, {});",
                    point_index(*point),
                    u8::from(*start)
                ));
            }
            IrCoverageStmt::SampleRegistered {
                group,
                strobe: true,
                ..
            } => {
                self.line(format!(
                    "llg_cov_strobe_registered(&{});",
                    group_name(*group)
                ));
            }
            IrCoverageStmt::SampleRegistered {
                group,
                function,
                depth,
                strobe: false,
            } => {
                let group = group_name(*group);
                self.begin_block(&[]);
                let index = self.scalar("size_t", "0".to_owned());
                let object = format!("_llg_cov_object_{index}");
                let binding = self.native_local(&object, NativeKind::Chandle);
                self.line(format!("while ({index} < llg_cov_registered(&{group})) {{"));
                self.line(format!(
                    "*({}) = llg_cov_registered_at(&{group}, {index}++);",
                    binding.address
                ));
                let mut call = IrCall::new(*function, Vec::new(), *depth, Vec::new(), Vec::new());
                call.receiver = Some(IrChandleExpr::LocalRead(object));
                self.call_statement(&call)?;
                self.line("}");
                self.end_block();
            }
            IrCoverageStmt::Watch {
                object,
                waiter,
                label,
            } => {
                let object = self.chandle(object)?;
                let frame = self.declare(
                    "llg_frame_t*",
                    "watch_frame",
                    "llg_frame_new(1ULL)".to_owned(),
                );
                self.line(format!("llg_frame_capture_opaque({frame}, 0u, {object});"));
                self.line(format!(
                    "llg_spawn_owned_with_frame(&{waiter}_desc, {}, {frame});",
                    c_string_literal(label)
                ));
                self.line(format!("llg_frame_release({frame});"));
            }
            IrCoverageStmt::SampleInstance {
                group,
                record,
                strobe: true,
                object,
                ..
            } => {
                let record = self.chandle(record)?;
                let object = self.chandle(object)?;
                self.line(format!(
                    "llg_cov_strobe(&{}, {record}, {object});",
                    group_name(*group)
                ));
            }
            IrCoverageStmt::SampleInstance {
                record,
                object,
                function,
                depth,
                strobe: false,
                ..
            } => {
                // A stopped instance skips the whole sample (19.8 stop()).
                self.begin_block(&[]);
                let record = self.chandle(record)?;
                self.line(format!("if (llg_cov_sampling({record})) {{"));
                let mut call = IrCall::new(*function, Vec::new(), *depth, Vec::new(), Vec::new());
                call.receiver = Some(object.clone());
                self.call_statement(&call)?;
                self.line("}");
                self.end_block();
            }
        }
        Ok(())
    }

    pub(super) fn coverage_query(&mut self, query: &IrCoverageQuery) -> Result<Value, String> {
        let (width, signed) = query.result_type();
        let code = match query {
            IrCoverageQuery::Instance { record, point } => {
                let record = self.chandle(record)?;
                format!("llg_cov_inst_coverage({record}, {})", point_index(*point))
            }
            IrCoverageQuery::Type {
                coverage_type,
                point,
            } => format!(
                "llg_cov_type_coverage(&{}, {})",
                type_name(*coverage_type),
                point_index(*point)
            ),
            IrCoverageQuery::Total => "llg_cov_total_coverage()".to_owned(),
            IrCoverageQuery::Option {
                record,
                point,
                option,
            } => {
                let record = self.chandle(record)?;
                format!(
                    "sv4_from_u64((uint64_t)llg_cov_option_get({record}, {}, {}u), {width}, {})",
                    point_index(*point),
                    option.code(),
                    u8::from(signed)
                )
            }
            IrCoverageQuery::TypeOption {
                coverage_type,
                point,
                option,
            } => format!(
                "sv4_from_u64((uint64_t)llg_cov_type_option_get(&{}, {}, {}u), {width}, {})",
                type_name(*coverage_type),
                point_index(*point),
                option.code(),
                u8::from(signed)
            ),
        };
        Ok(self.value(code, width, signed))
    }

    /// The two-argument coverage methods (19.8): the coverage, then the
    /// covered and total bin numbers stored into the `ref int` actuals.
    /// A coverage access function of 40.3.2: the integral arguments as
    /// value descriptors (the runtime rejects unknown bits), the scope as an
    /// instance index (-1 `$root`, -2 a definition name in `text`) and the
    /// database name of merge and save in `text`.
    pub(super) fn coverage_api(&mut self, call: &IrCoverageApiCall) -> Result<Value, String> {
        let mut values = Vec::with_capacity(call.args.len());
        for arg in &call.args {
            let value = self.expression(arg)?;
            if value.width == 0 {
                return Err("coverage access arguments must be integral".to_owned());
            }
            values.push(value);
        }
        let entries = values
            .iter()
            .map(|value| value.code.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        let array = self.declare_array_init("sv4_t", "coverage_args", values.len(), &entries);
        let (instance, text) = match (&call.scope, &call.name) {
            (Some(IrCoverageScope::Instance(index)), _) => (i64::from(*index), None),
            (Some(IrCoverageScope::Root), _) => (-1, None),
            (Some(IrCoverageScope::Definition(name)), _) => (-2, Some(self.string(name)?)),
            (None, Some(name)) => (-1, Some(self.string(name)?)),
            (None, None) => return Err("coverage access function without operand".to_owned()),
        };
        let (text_code, has_text) = match &text {
            Some(text) => (text.take_string(), 1),
            None => ("(llg_string_t){0}".to_owned(), 0),
        };
        let result = self.value(
            format!(
                "sv4_from_i64((int64_t)llg_coverage_call({}, {array}, {}, {instance}, {text_code}, {has_text}, {}), 32)",
                call.function.runtime_name(),
                values.len(),
                c_string_literal(&call.site)
            ),
            32,
            true,
        );
        if let Some(text) = text {
            self.native_discard(text);
        }
        for value in values {
            self.discard(value);
        }
        Ok(result)
    }

    pub(super) fn coverage_counts(
        &mut self,
        query: &IrCoverageQuery,
        covered: &IrPlusArgTarget,
        total: &IrPlusArgTarget,
    ) -> Result<Value, String> {
        let (
            IrPlusArgTarget::Packed {
                lhs: covered_lhs, ..
            },
            IrPlusArgTarget::Packed { lhs: total_lhs, .. },
        ) = (covered, total)
        else {
            return Err("coverage bin counts need packed targets".to_owned());
        };
        let covered_count = self.scalar("int32_t", "0".to_owned());
        let total_count = self.scalar("int32_t", "0".to_owned());
        let code = match query {
            IrCoverageQuery::Instance { record, point } => {
                let record = self.chandle(record)?;
                format!(
                    "llg_cov_inst_coverage_counts({record}, {}, &{covered_count}, &{total_count})",
                    point_index(*point)
                )
            }
            IrCoverageQuery::Type {
                coverage_type,
                point,
            } => format!(
                "llg_cov_type_coverage_counts(&{}, {}, &{covered_count}, &{total_count})",
                type_name(*coverage_type),
                point_index(*point)
            ),
            _ => return Err("coverage bin counts need a coverage query".to_owned()),
        };
        let result = self.value(code, 0, true);
        for (lhs, count) in [(covered_lhs, &covered_count), (total_lhs, &total_count)] {
            let target = self.target(lhs)?;
            let value = self.value(
                format!("sv4_from_u64((uint64_t)(int64_t){count}, 32, 1)"),
                32,
                true,
            );
            self.store(&target, value, false, "0")?;
            self.release_target(target);
        }
        Ok(result)
    }

    /// A string option read (19.7, 19.7.1).
    pub(super) fn coverage_text(
        &mut self,
        query: &IrCoverageTextQuery,
    ) -> Result<super::native::NativeValue, String> {
        let code = match query {
            IrCoverageTextQuery::Instance {
                record,
                point,
                which,
            } => {
                let record = self.chandle(record)?;
                format!(
                    "llg_cov_text_get({record}, {}, {}u)",
                    point_index(*point),
                    which.code()
                )
            }
            IrCoverageTextQuery::Type {
                coverage_type,
                point,
            } => format!(
                "llg_cov_type_text_get(&{}, {})",
                type_name(*coverage_type),
                point_index(*point)
            ),
        };
        Ok(self.native_value(NativeKind::String, code))
    }
}
