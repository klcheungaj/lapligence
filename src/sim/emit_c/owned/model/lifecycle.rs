//! Embeddable start/advance/close API. Suspension does not destroy live owners.
use super::*;
use crate::sim::execution::ScheduleRegion;
use std::collections::HashMap;

/// Startup calls whose arguments are compile-time constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StartupKind {
    WaveSv4,
    WaveReal,
    Spawn,
    ProgramSpawn,
    InstanceSpawn,
    InstanceProgramSpawn,
    ClassSpawn,
    Final,
    /// `llg_vpi_procedures_started`, between the procedures and the
    /// deferred always_comb/always_latch spawns.
    ProceduresStarted,
}

impl StartupKind {
    fn function(self) -> &'static str {
        match self {
            Self::WaveSv4 => "llg_wave_register_sv4",
            Self::WaveReal => "llg_wave_register_real",
            Self::Spawn => "llg_spawn_in_region",
            Self::ProgramSpawn => "llg_spawn_program_in_region",
            Self::InstanceSpawn => "llg_spawn_instance_in_region",
            Self::InstanceProgramSpawn => "llg_spawn_program_instance_in_region",
            Self::ClassSpawn => "llg_model_spawn_class",
            Self::Final => "llg_spawn_final",
            Self::ProceduresStarted => "llg_vpi_procedures_started",
        }
    }

    /// Argument table members in call order: (member declaration, field).
    fn fields(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::WaveSv4 => &[
                ("const char* name", "name"),
                ("sv4_t* value", "value"),
                ("uint32_t width", "width"),
            ],
            Self::WaveReal => &[("const char* name", "name"), ("double* value", "value")],
            Self::Spawn => &[
                ("const llg_co_desc_t* desc", "desc"),
                ("const char* name", "name"),
                ("llg_region_t region", "region"),
            ],
            Self::ProgramSpawn => &[
                ("const llg_co_desc_t* desc", "desc"),
                ("const char* name", "name"),
                ("llg_region_t region", "region"),
                ("uint64_t instance", "instance"),
                ("int initial", "initial"),
            ],
            Self::InstanceSpawn => &[
                ("const llg_co_desc_t* desc", "desc"),
                ("const char* name", "name"),
                ("llg_region_t region", "region"),
                ("const void* record", "record"),
                ("size_t record_offset", "record_offset"),
            ],
            Self::InstanceProgramSpawn => &[
                ("const llg_co_desc_t* desc", "desc"),
                ("const char* name", "name"),
                ("llg_region_t region", "region"),
                ("uint64_t instance", "instance"),
                ("int initial", "initial"),
                ("const void* record", "record"),
                ("size_t record_offset", "record_offset"),
            ],
            Self::ClassSpawn => &[
                ("uint32_t body_class", "body_class"),
                ("uint32_t first", "first"),
                ("uint32_t count", "count"),
            ],
            Self::Final => &[("void (*fn)(void)", "fn"), ("const char* name", "name")],
            Self::ProceduresStarted => &[],
        }
    }

    fn table_type(self) -> &'static str {
        match self {
            Self::WaveSv4 => "llg_model_wave_sv4_args_t",
            Self::WaveReal => "llg_model_wave_real_args_t",
            Self::Spawn => "llg_model_spawn_args_t",
            Self::ProgramSpawn => "llg_model_program_spawn_args_t",
            Self::InstanceSpawn => "llg_model_instance_spawn_args_t",
            Self::InstanceProgramSpawn => "llg_model_program_instance_spawn_args_t",
            Self::ClassSpawn => "llg_model_class_spawn_args_t",
            Self::Final => "llg_model_final_args_t",
            Self::ProceduresStarted => "llg_model_procedures_started_args_t",
        }
    }

    /// Registration failures abort startup; spawns report fatal errors
    /// themselves.
    fn checked(self) -> bool {
        matches!(self, Self::WaveSv4 | Self::WaveReal)
    }
}

struct StartupCall {
    kind: StartupKind,
    args: Vec<String>,
}

/// Root processes that share one body (`model/sharing.rs`), grouped by
/// class. Each member's instance record is one row of the class's record
/// array.
#[derive(Default)]
pub(in crate::sim::emit_c) struct SharedSpawns {
    pub(in crate::sim::emit_c) classes: Vec<SpawnClass>,
    /// Spawned process C name to its class and member index.
    pub(in crate::sim::emit_c) members: HashMap<String, (usize, usize)>,
}

pub(in crate::sim::emit_c) struct SpawnClass {
    pub(in crate::sim::emit_c) body: String,
    pub(in crate::sim::emit_c) records: String,
    pub(in crate::sim::emit_c) record_type: String,
    pub(in crate::sim::emit_c) frame: String,
    /// Varying `const char*` record fields with their value per member: a
    /// spawn label found here is read from the record instead of a table.
    pub(in crate::sim::emit_c) string_fields: Vec<(String, Vec<String>)>,
    pub(in crate::sim::emit_c) members: Vec<crate::sim::execution::CoroutineId>,
    /// One descriptor for every member, set when the members' resume-site
    /// tables are identical; otherwise each member keeps its own.
    pub(in crate::sim::emit_c) desc: Option<String>,
}

/// Emits startup calls in their original order. A run of consecutive calls of
/// one kind becomes a static argument table and a loop, so large designs do
/// not repeat one call statement per signal or process; a lone call stays a
/// direct call.
#[derive(Default)]
struct StartupTables {
    source: String,
    count: usize,
    declared: Vec<StartupKind>,
}

impl StartupTables {
    fn render(&mut self, calls: &[StartupCall], body: &mut String) {
        let mut start = 0;
        while start < calls.len() {
            let kind = calls[start].kind;
            let end = calls[start..]
                .iter()
                .position(|call| call.kind != kind)
                .map_or(calls.len(), |offset| start + offset);
            let run = &calls[start..end];
            if run.len() == 1 {
                let call = format!("{}({})", kind.function(), run[0].args.join(", "));
                body.push_str(&Self::statement(kind, &call));
            } else {
                let table = self.table(kind, run);
                let args = kind
                    .fields()
                    .iter()
                    .map(|(_, field)| format!("{table}[_llg_n].{field}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                let call = format!("{}({args})", kind.function());
                body.push_str(&format!(
                    "    for (size_t _llg_n = 0; _llg_n < sizeof({table}) / sizeof({table}[0]); ++_llg_n)\n    {}",
                    Self::statement(kind, &call)
                ));
            }
            start = end;
        }
    }

    fn statement(kind: StartupKind, call: &str) -> String {
        if kind.checked() {
            format!("    if ({call} != 0) goto start_failed;\n")
        } else {
            format!("    {call};\n")
        }
    }

    fn table(&mut self, kind: StartupKind, run: &[StartupCall]) -> String {
        if !self.declared.contains(&kind) {
            self.declared.push(kind);
            let members = kind
                .fields()
                .iter()
                .map(|(member, _)| format!(" {member};"))
                .collect::<String>();
            self.source.push_str(&format!(
                "typedef struct {{{members} }} {};\n",
                kind.table_type()
            ));
        }
        let table = format!("llg_model_startup_{}", self.count);
        self.count += 1;
        self.source.push_str(&format!(
            "static const {} {table}[{}] = {{\n",
            kind.table_type(),
            run.len()
        ));
        for call in run {
            self.source
                .push_str(&format!("    {{ {} }},\n", call.args.join(", ")));
        }
        self.source.push_str("};\n");
        table
    }
}

/// The class table and spawn helper of shared root processes that have one
/// class descriptor. A member's spawn label is read from a record field
/// holding it for every member, else from a per-class label table.
fn render_spawn_classes(
    sharing: &SharedSpawns,
    labels: &HashMap<&str, &str>,
    regions: &[Option<String>],
    out: &mut String,
) -> Vec<Option<usize>> {
    let mut indices = Vec::with_capacity(sharing.classes.len());
    let mut rows = Vec::new();
    for (class_index, class) in sharing.classes.iter().enumerate() {
        let (Some(desc), Some(region)) = (&class.desc, &regions[class_index]) else {
            indices.push(None);
            continue;
        };
        let mut members = vec![""; class.members.len()];
        for (name, (index, member)) in &sharing.members {
            if *index == class_index {
                members[*member] = labels.get(name.as_str()).copied().unwrap_or("");
            }
        }
        let members = members
            .iter()
            .map(|label| c_string_literal(label))
            .collect::<Vec<_>>();
        let field = class
            .string_fields
            .iter()
            .find(|(_, values)| *values == members)
            .map(|(field, _)| field);
        let names = match field {
            Some(_) => "NULL".to_owned(),
            None => {
                let table = format!("{}_labels", class.records);
                out.push_str(&format!(
                    "static const char* const {table}[{}] = {{ {} }};\n",
                    members.len(),
                    members.join(", ")
                ));
                table
            }
        };
        let name_offset = field.map_or_else(
            || "0".to_owned(),
            |field| format!("offsetof({}, {field})", class.record_type),
        );
        indices.push(Some(rows.len()));
        rows.push(format!(
            "    {{ &{desc}, {}, sizeof({}), offsetof({}, _llg_instance), {name_offset}, {names}, {region} }},\n",
            class.records, class.record_type, class.frame
        ));
    }
    if rows.is_empty() {
        return indices;
    }
    out.push_str(
        "typedef struct { const llg_co_desc_t* desc; const void* records; size_t stride; size_t record_offset; size_t name_offset; const char* const* names; llg_region_t region; } llg_model_body_class_t;\n",
    );
    out.push_str(&format!(
        "static const llg_model_body_class_t llg_model_body_classes[{}] = {{\n{}}};\n",
        rows.len(),
        rows.concat()
    ));
    out.push_str(
        "static void llg_model_spawn_class(uint32_t body_class, uint32_t first, uint32_t count) {\n    \
         const llg_model_body_class_t* c = &llg_model_body_classes[body_class];\n    \
         for (uint32_t m = first; m < first + count; ++m) {\n        \
         const char* record = (const char*)c->records + (size_t)m * c->stride;\n        \
         const char* name = c->names ? c->names[m] : *(const char* const*)(const void*)(record + c->name_offset);\n        \
         llg_spawn_instance_in_region(c->desc, name, c->region, record, c->record_offset);\n    \
         }\n}\n",
    );
    indices
}

/// Merges consecutive class spawns of one class whose members follow each
/// other into one call, keeping the spawn order.
fn merge_class_spawns(calls: Vec<StartupCall>) -> Vec<StartupCall> {
    let mut merged: Vec<StartupCall> = Vec::with_capacity(calls.len());
    for call in calls {
        if call.kind == StartupKind::ClassSpawn {
            if let Some(last) = merged.last_mut() {
                if last.kind == StartupKind::ClassSpawn && last.args[0] == call.args[0] {
                    let first = last.args[1].parse::<usize>().unwrap_or(usize::MAX);
                    let count = last.args[2].parse::<usize>().unwrap_or(usize::MAX);
                    if first
                        .checked_add(count)
                        .map(|end| end.to_string())
                        .as_deref()
                        == Some(call.args[1].as_str())
                    {
                        last.args[2] = (count + 1).to_string();
                        continue;
                    }
                }
            }
        }
        merged.push(call);
    }
    merged
}

pub(in crate::sim::emit_c) fn main(
    execution: &ExecutionModel,
    sharing: &SharedSpawns,
) -> Result<String, String> {
    let model = execution.ir();
    let mut tables = StartupTables::default();
    let mut out = String::from(
        "/* start: 0=ready, 1=error; advance: 0=complete, 1=error, 2=suspended.\n\
         * A suspended model retains all owners until another advance or close.\n\
         * Define LLG_MODEL_NO_MAIN to drive these entry points from a host. */\n\
         static int llg_model_live, llg_model_done, llg_model_status;\n\
         int llg_model_close(void);\n",
    );
    if model.waveform {
        out.push_str("static int llg_model_wave_live;\n");
    }
    out.push_str(&format!("int llg_model_start(int argc, char** argv) {{\n    llg_value_require_abi();\n    if (llg_model_live) return 1;\n    llg_model_live = 1;\n    llg_model_done = llg_model_status = 0;\n    llg_rt_init_with_args_and_precision(argc, argv, {}ULL);\n    if (llg_rt_failed()) goto start_failed;\n", model.precision_fs));
    // Without a clocking drive, `##` wait or assertion clock pulse nothing
    // reads the runtime's current-slot edge history, so signal publication
    // skips recording it.
    if !model.clocking_history {
        out.push_str("    llg_rt_skip_clocking_history();\n");
    }
    out.push_str(&crate::sim::emit_c::owned::coverage::startup(model));
    if !model.classes.is_empty() {
        // Collector roots precede storage initializers, which may allocate.
        out.push_str("    if (!llg_model_gc_register()) {\n        fprintf(stderr, \"llg: allocation failed registering collector roots\\n\");\n        goto start_failed;\n    }\n");
    }
    // Static processes, and objects that static initializers create, seed
    // from their instance's initialization stream only where some stream is
    // observable (SV 18.14.1).
    if model.random.threads || model.random.objects {
        out.push_str("    llg_rt_use_instance_random_streams();\n");
    }
    out.push_str("    llg_model_storage_defaults();\n    llg_model_initializers();\n    if (llg_rt_failed()) goto start_failed;\n");
    out.push_str(&crate::sim::emit_c::owned::coverage::fsm_startup(model));
    out.push_str(&crate::sim::emit_c::owned::native::helpers_used());
    // Mark otherwise unused generated function definitions as intentional.
    for function in &model.funcs {
        if !inline_template(function) {
            out.push_str(&format!("    (void){};\n", function.c_name));
        }
    }
    // A coroutine subroutine with no caller (a timed task never called, or a
    // template whose callers all bind records, SIM-008) leaves its
    // descriptor unreferenced.
    for &index in execution.analysis().callee_first_functions() {
        out.push_str(&format!("    (void)&{}_desc;\n", model.funcs[index].c_name));
    }
    if !model.classes.is_empty() {
        out.push_str(
            "    (void)llg_class_field_quiet; (void)llg_class_packed_dependency; (void)llg_class_real_dependency; (void)llg_class_handle_dependency; (void)llg_class_string_dependency; (void)llg_class_handle_store; (void)llg_class_handle_published;\n",
        );
    }
    if !model.classes.is_empty() && model.random.objects {
        out.push_str("    (void)llg_class_rng;\n");
    }
    if !model.virtual_interfaces.is_empty() {
        out.push_str(
            "    (void)llg_vif_member; (void)llg_vif_read; (void)llg_vif_member_quiet; (void)llg_vif_member_dependency; (void)llg_vif_real_member; (void)llg_vif_real_member_quiet;\n",
        );
        for (interface_id, interface) in model.virtual_interfaces.iter().enumerate() {
            for instance in &interface.instances {
                out.push_str(&format!("    (void){};\n", instance.c_name));
            }
            for method in 0..interface.methods.len() {
                out.push_str(&format!(
                    "    (void)llg_vif_call_{interface_id}_{method};\n"
                ));
            }
        }
    }
    out.push_str("    if (!llg_model_assertions_init()) goto start_failed;\n");
    if model.waveform {
        out.push_str(&format!("    llg_wave_final_time = 0;\n    llg_model_wave_live = 1;\n    if (llg_wave_model_init({}ULL) != 0) goto start_failed;\n", model.precision_fs));
        let mut registrations = Vec::new();
        for (index, signal) in model.signals.iter().enumerate() {
            if signal.omit && signal.net_driver.is_none() && signal.net_alias.is_empty() {
                continue;
            }
            let Some(name) = &signal.hdl_name else {
                continue;
            };
            registrations.push(match signal.ty {
                IrType::Packed { width, .. } => StartupCall {
                    kind: StartupKind::WaveSv4,
                    args: vec![
                        c_string_literal(name),
                        if signal.net_alias.is_empty() {
                            format!("&{}", signal.c_name)
                        } else {
                            format!("&llg_net_alias_{index}.visible")
                        },
                        width.to_string(),
                    ],
                },
                IrType::Real { .. } => StartupCall {
                    kind: StartupKind::WaveReal,
                    args: vec![c_string_literal(name), format!("&{}", signal.c_name)],
                },
            });
        }
        for array in model.arrays.iter().filter(|array| !array.activation) {
            if array.sparse() {
                tables.render(&registrations, &mut out);
                registrations.clear();
                let mut format = array.hdl_name.replace('%', "%%");
                let mut coordinates = Vec::new();
                let mut stride = array.total;
                for (left, right) in &array.dims {
                    let extent = i64::from(*left).abs_diff(i64::from(*right)) + 1;
                    stride /= extent;
                    format.push_str("[%lld]");
                    coordinates.push(format!("(long long)((int64_t){left} {} (int64_t)((_llg_n / {stride}ULL) % {extent}ULL))", if left >= right { "-" } else { "+" }));
                }
                let capacity = array.hdl_name.len() + array.dims.len() * 14 + 1;
                out.push_str(&format!("    for (uint64_t _llg_n = 0; _llg_n < {}ULL; ++_llg_n) {{ char _llg_name[{capacity}]; snprintf(_llg_name, sizeof(_llg_name), {}, {}); if (llg_wave_register_sv4(_llg_name, {}, {}u) != 0) goto start_failed; }}\n", array.total, c_string_literal(&format), coordinates.join(", "), array.cell_address("_llg_n"), array.elem_width));
                continue;
            }
            for index in 0..array.total {
                let name = array
                    .waveform_element_name(index)
                    .ok_or_else(|| "invalid waveform array index".to_owned())?;
                registrations.push(if array.real {
                    StartupCall {
                        kind: StartupKind::WaveReal,
                        args: vec![
                            c_string_literal(&name),
                            format!("&{}[{index}]", array.c_name),
                        ],
                    }
                } else {
                    StartupCall {
                        kind: StartupKind::WaveSv4,
                        args: vec![
                            c_string_literal(&name),
                            format!("&{}[{index}]", array.c_name),
                            array.elem_width.to_string(),
                        ],
                    }
                });
            }
        }
        tables.render(&registrations, &mut out);
        if let Some(start) = &model.wave_start {
            let selection = start.selection();
            let names = if selection.names().is_empty() {
                "NULL".to_owned()
            } else {
                out.push_str(&format!(
                    "    static const char* const llg_wave_start_scopes[] = {{{}}};\n",
                    selection
                        .names()
                        .iter()
                        .map(|name| c_string_literal(name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
                "llg_wave_start_scopes".to_owned()
            };
            out.push_str(&format!(
                "    if (llg_wave_start({}, {}u, {names}, {}u) != 0) goto start_failed;\n",
                c_string_literal(start.file()),
                selection.depth(),
                selection.names().len()
            ));
        }
    }
    if !model.dpi_scopes().is_empty() {
        out.push_str(&format!(
            "    llg_dpi_model_init(llg_dpi_scopes, {}u);\n",
            model.dpi_scopes().len()
        ));
    }
    // How `vpi_put_delays` may write the model's delays (38.32).
    if model.vpi_catalog && (model.vpi_delays_writable || model.vpi_delay_select != 1) {
        out.push_str(&format!(
            "    llg_vpi_model_delays({}, {});\n",
            u8::from(model.vpi_delays_writable),
            model.vpi_delay_select
        ));
    }
    out.push_str(&format!("    if (!llg_vpi_model_init({}, llg_vpi_objects, llg_vpi_object_count) || !llg_vpi_model_types(llg_vpi_type_tables) || !llg_vpi_model_dynamic(llg_vpi_dynamic_tables) || !llg_vpi_model_code(llg_vpi_code_tables) || !llg_vpi_startup_static(LLG_VPI_STATIC_ROUTINES) || !llg_vpi_startup()) goto start_failed;\n", c_string_literal(model.design_name())));
    if let Some(check) = crate::sim::emit_c::model::vpi::vpi_builtin_check(model) {
        out.push_str(&format!("    if (!{check}) goto start_failed;\n"));
    }
    let scopes = crate::sim::emit_c::model::vpi::vpi_call_scopes(model);
    for ((index, call), scope) in model.vpi_compile_calls.iter().enumerate().zip(&scopes) {
        out.push_str(&format!("    if (!llg_vpi_compile_call_site_at({index}ULL, {}, llg_vpi_compile_args_{index}, {}, {}ULL, {scope}, {}, {})) goto start_failed;\n",
            c_string_literal(&call.name), call.args.len(), call.time_unit_fs,
            call.file.as_deref().map_or_else(|| "NULL".to_owned(), c_string_literal), call.line));
    }
    out.push_str("    llg_vpi_start_simulation();\n    if (llg_vpi_failed()) goto start_failed;\n");
    // First executable process per C name, matching the former linear search.
    let mut executable_by_name = HashMap::new();
    for item in execution.processes() {
        executable_by_name
            .entry(model.processes[item.semantic_process].c_name.as_str())
            .or_insert(item);
    }
    let mut spawns = Vec::new();
    for (name, fallback_label) in model.spawn_list() {
        let process = executable_by_name.get(name).copied();
        let semantic = process.map(|item| &model.processes[item.semantic_process]);
        let region = process
            .map(|item| item.region)
            .unwrap_or(ScheduleRegion::Active);
        let label = semantic.map(|item| item.label()).unwrap_or(fallback_label);
        if let Some(instance) = semantic.and_then(|item| item.program) {
            let initial = semantic.is_some_and(|item| item.kind() == IrProcessKind::Initial);
            spawns.push(StartupCall {
                kind: StartupKind::ProgramSpawn,
                args: vec![
                    format!("&{name}_desc"),
                    c_string_literal(label),
                    region.runtime_symbol().to_owned(),
                    format!("{instance}ULL"),
                    u8::from(initial).to_string(),
                ],
            });
        } else {
            spawns.push(StartupCall {
                kind: StartupKind::Spawn,
                args: vec![
                    format!("&{name}_desc"),
                    c_string_literal(label),
                    region.runtime_symbol().to_owned(),
                ],
            });
        }
    }
    // Spawn labels as written, keyed by process name.
    let mut labels = HashMap::new();
    for (name, fallback_label) in model.spawn_list() {
        let label = executable_by_name
            .get(name)
            .map(|item| model.processes[item.semantic_process].label())
            .unwrap_or(fallback_label);
        labels.entry(name).or_insert(label);
    }
    // A class spawns in the region of its first spawned member; members in
    // another region, and program processes, keep their own startup rows.
    let mut regions = vec![None; sharing.classes.len()];
    for (call, name) in spawns.iter().zip(&model.spawns) {
        if let Some(&(class_index, _)) = sharing.members.get(name) {
            if call.kind == StartupKind::Spawn && regions[class_index].is_none() {
                regions[class_index] = Some(call.args[2].clone());
            }
        }
    }
    let class_rows = render_spawn_classes(sharing, &labels, &regions, &mut tables.source);
    for (call, name) in spawns.iter_mut().zip(&model.spawns) {
        let Some(&(class_index, member)) = sharing.members.get(name) else {
            continue;
        };
        let class = &sharing.classes[class_index];
        match (call.kind, class_rows[class_index]) {
            (StartupKind::Spawn, Some(row))
                if regions[class_index].as_deref() == Some(call.args[2].as_str()) =>
            {
                call.kind = StartupKind::ClassSpawn;
                call.args = vec![row.to_string(), member.to_string(), "1".to_owned()];
            }
            (kind, _) => {
                call.kind = if kind == StartupKind::ProgramSpawn {
                    StartupKind::InstanceProgramSpawn
                } else {
                    StartupKind::InstanceSpawn
                };
                if let Some(desc) = &class.desc {
                    call.args[0] = format!("&{desc}");
                }
                call.args.extend([
                    format!("&{}[{member}]", class.records),
                    format!("offsetof({}, _llg_instance)", class.frame),
                ]);
            }
        }
    }
    // always_comb and always_latch spawn last (lowering, 9.2.2.2); the
    // runtime is told when every other time-zero procedure is queued.
    let deferred_start = model
        .spawns
        .iter()
        .position(|name| {
            executable_by_name.get(name.as_str()).is_some_and(|item| {
                matches!(
                    model.processes[item.semantic_process].kind(),
                    IrProcessKind::Comb | IrProcessKind::Latch
                )
            })
        })
        .unwrap_or(model.spawns.len());
    let deferred = spawns.split_off(deferred_start);
    let mut spawns = merge_class_spawns(spawns);
    spawns.push(StartupCall {
        kind: StartupKind::ProceduresStarted,
        args: Vec::new(),
    });
    spawns.extend(merge_class_spawns(deferred));
    let mut semantic_by_name = HashMap::new();
    for process in &model.processes {
        semantic_by_name
            .entry(process.c_name.as_str())
            .or_insert(process);
    }
    if model.waveform {
        spawns.push(StartupCall {
            kind: StartupKind::Final,
            args: vec![
                "llg_wave_capture_final_time".to_owned(),
                "\"llg.wave.capture_final_time\"".to_owned(),
            ],
        });
    }
    for name in &model.final_spawns {
        let label = semantic_by_name
            .get(name.as_str())
            .map(|p| p.label())
            .unwrap_or("unnamed final process");
        spawns.push(StartupCall {
            kind: StartupKind::Final,
            args: vec![name.clone(), c_string_literal(label)],
        });
    }
    tables.render(&spawns, &mut out);
    out.push_str(
        "    return 0;\nstart_failed:\n    (void)llg_model_close();\n    return 1;\n}\n\n",
    );
    out.push_str("int llg_model_advance(void) {\n    if (!llg_model_live) return 1;\n    if (llg_model_done) return llg_model_status;\n    if (llg_rt_is_suspended() && !llg_rt_resume()) return 1;\n    llg_rt_run();\n    if (llg_rt_is_suspended()) return 2;\n");
    if model.waveform || !model.final_spawns.is_empty() {
        out.push_str("    llg_rt_run_finals();\n");
    }
    out.push_str("    llg_vpi_end_simulation();\n    llg_model_done = 1;\n    llg_model_status = (llg_rt_failed() || llg_vpi_failed()) ? 1 : 0;\n    return llg_model_status;\n}\n\n");
    out.push_str(
        "int llg_model_close(void) {\n    int status = 0;\n    if (!llg_model_live) return 0;\n",
    );
    if model.waveform {
        out.push_str("    if (llg_model_wave_live) {\n        status = llg_wave_close(llg_model_done ? llg_wave_final_time : llg_time()) != 0;\n        llg_model_wave_live = 0;\n    }\n");
    }
    out.push_str(
        "    llg_vpi_shutdown();\n    llg_rt_cleanup();\n    llg_model_storage_destroy();\n",
    );
    if !model.native_values.is_empty() {
        // Every native root belongs to model storage or an unwound scope.
        out.push_str("    if (llg_native_roots_count() != 0) {\n        fprintf(stderr, \"llg: %zu native values were not released at model close\\n\", llg_native_roots_count());\n        status = 1;\n    }\n");
    }
    out.push_str("    llg_model_live = llg_model_done = llg_model_status = 0;\n    return status;\n}\n\n#ifndef LLG_MODEL_NO_MAIN\nint main(int argc, char** argv) {\n    int status = llg_model_start(argc, argv);\n    if (status == 0) {\n        /* A start callback's exit-policy vpiStop suspends before the run. */\n        status = llg_rt_is_suspended() ? 2 : llg_model_advance();\n        if (status == 2) status = 0; /* CLI exit-policy stop, not a model error. */\n    }\n    if (llg_model_close() != 0) status = 1;\n    return status;\n}\n#endif\n");
    // Startup argument tables are file-scope constants read by the loops.
    let start = out
        .find("int llg_model_start(")
        .ok_or_else(|| "missing llg_model_start definition".to_owned())?;
    out.insert_str(start, &tables.source);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{StartupCall, StartupKind, StartupTables};

    fn spawn(name: &str) -> StartupCall {
        StartupCall {
            kind: StartupKind::Spawn,
            args: vec![
                format!("&{name}_desc"),
                format!("\"{name}\""),
                "LLG_REGION_ACTIVE".to_owned(),
            ],
        }
    }

    #[test]
    fn runs_become_ordered_tables_and_single_calls_stay_direct() {
        let calls = vec![
            spawn("p_a"),
            spawn("p_b"),
            StartupCall {
                kind: StartupKind::Final,
                args: vec!["f_only".to_owned(), "\"f_only\"".to_owned()],
            },
            spawn("p_c"),
            spawn("p_d"),
            spawn("p_e"),
        ];
        let mut tables = StartupTables::default();
        let mut body = String::new();
        tables.render(&calls, &mut body);

        assert_eq!(tables.source.matches("typedef struct {").count(), 1);
        let first = tables.source.find("llg_model_startup_0[2]").unwrap();
        let second = tables.source.find("llg_model_startup_1[3]").unwrap();
        assert!(first < second);
        let rows = [
            "&p_a_desc",
            "&p_b_desc",
            "&p_c_desc",
            "&p_d_desc",
            "&p_e_desc",
        ]
        .map(|row| tables.source.find(row).unwrap());
        assert!(
            rows.windows(2).all(|pair| pair[0] < pair[1]),
            "{}",
            tables.source
        );

        let loop0 = body.find("llg_model_startup_0[_llg_n].desc").unwrap();
        let direct = body.find("llg_spawn_final(f_only, \"f_only\");").unwrap();
        let loop1 = body.find("llg_model_startup_1[_llg_n].desc").unwrap();
        assert!(loop0 < direct && direct < loop1, "{body}");
    }

    #[test]
    fn waveform_registrations_keep_their_failure_check() {
        let calls = ["a", "b"]
            .map(|name| StartupCall {
                kind: StartupKind::WaveSv4,
                args: vec![format!("\"{name}\""), format!("&G_{name}"), "4".to_owned()],
            })
            .into_iter()
            .chain([StartupCall {
                kind: StartupKind::WaveReal,
                args: vec!["\"r\"".to_owned(), "&D_r".to_owned()],
            }])
            .collect::<Vec<_>>();
        let mut tables = StartupTables::default();
        let mut body = String::new();
        tables.render(&calls, &mut body);

        assert!(body.contains(
            "if (llg_wave_register_sv4(llg_model_startup_0[_llg_n].name, llg_model_startup_0[_llg_n].value, llg_model_startup_0[_llg_n].width) != 0) goto start_failed;"
        ), "{body}");
        assert!(
            body.contains("if (llg_wave_register_real(\"r\", &D_r) != 0) goto start_failed;"),
            "{body}"
        );
        assert!(
            tables.source.contains("{ \"a\", &G_a, 4 },"),
            "{}",
            tables.source
        );
    }
}
