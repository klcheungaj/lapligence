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
    Final,
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
            Self::Final => "llg_spawn_final",
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
            Self::Final => &[("void (*fn)(void)", "fn"), ("const char* name", "name")],
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
            Self::Final => "llg_model_final_args_t",
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

pub(in crate::sim::emit_c) fn main(
    execution: &ExecutionModel,
    sharing: &HashMap<String, (String, String)>,
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
    if !model.classes.is_empty() {
        // Collector roots precede storage initializers, which may allocate.
        out.push_str("    if (!llg_model_gc_register()) {\n        fprintf(stderr, \"llg: allocation failed registering collector roots\\n\");\n        goto start_failed;\n    }\n");
    }
    out.push_str("    llg_model_storage_defaults();\n    llg_model_initializers();\n    if (llg_rt_failed()) goto start_failed;\n");
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
            "    (void)llg_class_field_quiet; (void)llg_class_packed_dependency; (void)llg_class_real_dependency; (void)llg_class_handle_dependency; (void)llg_class_string_dependency; (void)llg_class_handle_store;\n",
        );
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
    out.push_str(&format!("    if (!llg_vpi_model_init({}, llg_vpi_objects, llg_vpi_object_count) || !llg_vpi_startup()) goto start_failed;\n", c_string_literal(model.design_name())));
    for (index, call) in model.vpi_compile_calls.iter().enumerate() {
        out.push_str(&format!("    if (!llg_vpi_compile_call_site({index}ULL, {}, llg_vpi_compile_args_{index}, {}, {}ULL)) goto start_failed;\n",
            c_string_literal(&call.name), call.args.len(), call.time_unit_fs));
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
    for (call, name) in spawns.iter_mut().zip(&model.spawns) {
        if let Some((record, offset)) = sharing.get(name) {
            call.kind = if call.kind == StartupKind::ProgramSpawn {
                StartupKind::InstanceProgramSpawn
            } else {
                StartupKind::InstanceSpawn
            };
            call.args.extend([record.clone(), offset.clone()]);
        }
    }
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
    out.push_str("    llg_model_live = llg_model_done = llg_model_status = 0;\n    return status;\n}\n\n#ifndef LLG_MODEL_NO_MAIN\nint main(int argc, char** argv) {\n    int status = llg_model_start(argc, argv);\n    if (status == 0) {\n        status = llg_model_advance();\n        if (status == 2) status = 0; /* CLI exit-policy stop, not a model error. */\n    }\n    if (llg_model_close() != 0) status = 1;\n    return status;\n}\n#endif\n");
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
