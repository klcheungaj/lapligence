//! Functional coverage (IEEE 1800-2009 19): static coverpoint tables and
//! the statements and queries that drive `coverage/coverage.c`.
use super::native::NativeKind;
use super::*;
use crate::sim::ir::{
    IrCoverAutomatic, IrCoverBinShape, IrCoverBound, IrCoverCount, IrCoverageQuery, IrCoverageStmt,
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

/// Static metadata of every covergroup; nothing for a model without one.
pub(in crate::sim::emit_c) fn tables(model: &IrModel, out: &mut String) {
    for (index, coverage_type) in model.coverage_types.iter().enumerate() {
        out.push_str(&format!(
            "static LLG_MAYBE_UNUSED llg_cov_type_t {} = {{ .name = {}, .weight = INT64_C({}) }};\n",
            type_name(index),
            c_string_literal(&coverage_type.name),
            coverage_type.weight
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
            let items_name = if point.items.is_empty() {
                "NULL".to_owned()
            } else {
                let rows = point
                    .items
                    .iter()
                    .map(|item| {
                        let (lo_kind, lo) = bound(item.lo);
                        let (hi_kind, hi) = bound(item.hi);
                        format!(
                            "{{ UINT64_C({lo}), UINT64_C({hi}), {lo_kind}, {hi_kind}, {} }}",
                            optional_string(item.site.as_deref())
                        )
                    })
                    .collect::<Vec<_>>();
                out.push_str(&format!(
                    "static const llg_cov_item_t {point_prefix}_items[] = {{\n    {}\n}};\n",
                    rows.join(",\n    ")
                ));
                format!("{point_prefix}_items")
            };
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
            points.push(format!(
                "{{ .name = {}, .width = {}u, .is_signed = {}u, .kind = {kind}, .dynamic = {}u, .n_keys = {}u, .keys = {keys_name}, .n_bins = {}u, .bins = {bins_name}, .items = {items_name} }}",
                c_string_literal(&point.name),
                point.width,
                u8::from(point.signed),
                u8::from(point.dynamic()),
                keys.map_or(0, Vec::len),
                point.bins.len(),
            ));
        }
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
            "// Unreferenced when every handle of the group stays null.\nstatic LLG_MAYBE_UNUSED llg_cov_group_t {prefix} = {{ .name = {}, .n_points = {}u, .points = {points_name}, .n_slots = {}u, .type = &{}, .strobe_sample = {strobe} }};\n",
            c_string_literal(&group.name),
            group.points.len(),
            group.slots,
            type_name(group.coverage_type)
        ));
    }
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
        let code = match query {
            IrCoverageQuery::Instance { record, point } => {
                let record = self.chandle(record)?;
                format!("llg_cov_inst_coverage({record}, {})", point_index(*point))
            }
            IrCoverageQuery::Type {
                coverage_type,
                point,
                weight,
            } => format!(
                "llg_cov_type_coverage(&{}, {}, INT64_C({weight}))",
                type_name(*coverage_type),
                point_index(*point)
            ),
            IrCoverageQuery::Total => "llg_cov_total_coverage()".to_owned(),
        };
        Ok(self.value(code, 0, true))
    }
}
