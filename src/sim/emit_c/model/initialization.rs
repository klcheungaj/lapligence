//! Initialization.

use super::*;

#[allow(dead_code)] // legacy model renderer superseded by owned::model
pub(super) fn render_main(execution: &ExecutionModel) -> Result<String, String> {
    use crate::sim::ir::IrInitStep;
    let model = execution.ir();
    let ctx = RCtx {
        model,
        func: None,
        sampled: false,
        activation_label: None,
    };
    let mut out = format!(
        "int main(int argc, char** argv) {{\n    llg_rt_init_with_args_precision_and_stack(argc, argv, {}ULL, LLG_MODEL_STACK_VALUES);\n    if (llg_rt_failed()) {{\n        llg_rt_cleanup();\n        return 1;\n    }}\n",
        model.precision_fs
    );
    for (index, signal) in model.signals.iter().enumerate() {
        if !signal.net_alias.is_empty() {
            out.push_str(&format!(
                "    llg_net_alias_bind(&llg_net_alias_{index});\n"
            ));
        }
    }
    for array in &model.arrays {
        let bind_element = if array.real {
            format!(
                "llg_dependency_bind_real(&{}[_i], &{}_llg_element_deps[_i]);",
                array.c_name, array.c_name
            )
        } else {
            format!(
                "llg_dependency_bind(&{}[_i], &{}_llg_element_deps[_i]);",
                array.c_name, array.c_name
            )
        };
        let bind_contents = if array.real {
            format!(
                "llg_dependency_bind_real(&{}[_i], &{}_llg_contents_dep);",
                array.c_name, array.c_name
            )
        } else {
            format!(
                "llg_dependency_bind(&{}[_i], &{}_llg_contents_dep);",
                array.c_name, array.c_name
            )
        };
        out.push_str(&format!(
            "    for (uint64_t _i = 0; _i < {}; ++_i) {{\n\
                     {}_llg_element_deps[_i] = SV4_C(0, 1);\n\
                     {bind_element}\n\
                     {bind_contents}\n\
                 }}\n",
            array.total, array.c_name
        ));
    }
    for object in &model.objects {
        if object.ty == crate::sim::ir::IrObjectType::String {
            out.push_str(&format!(
                "    {}.notify = llg_dependency_changed;\n    {}.dependency = &{}_llg_dep;\n",
                object.c_name, object.c_name, object.c_name
            ));
        }
    }
    for container in &model.containers {
        out.push_str(&super::super::containers::declaration_and_init(container)?.1);
        if let Some(size) = container.initial_size {
            out.push_str(&format!(
                "    llg_dyn_value_new(&{}, sv4_from_u64({}, 32, 1), NULL);\n",
                container.c_name, size
            ));
        }
    }
    for step in &model.init_steps {
        match step {
            IrInitStep::FillArrayX(arr) => {
                let a = model.array(*arr);
                let value = if a.real {
                    "0.0".to_string()
                } else {
                    packed_default(a.elem_width, a.signed, a.two_state)
                };
                out.push_str(&format!(
                    "    {{ for (uint64_t _i = 0; _i < {}; _i++) {}[_i] = {}; }}\n",
                    a.total, a.c_name, value
                ));
            }
            IrInitStep::FillArrayZ(arr) => {
                let a = model.array(*arr);
                if a.real {
                    return Err("real arrays cannot be initialized with Z".to_string());
                }
                out.push_str(&format!(
                    "    {{ for (uint64_t _i = 0; _i < {}; _i++) {}[_i] = sv4_fill(3, {}, {}); }}\n",
                    a.total, a.c_name, a.elem_width, a.signed as u8
                ));
            }
            IrInitStep::SetArrayElem { arr, index, value } => {
                let a = model.array(*arr);
                if a.real {
                    out.push_str(&format!(
                        "    {}[{}] = {};\n",
                        a.c_name,
                        index,
                        round_shortreal(emit_const_for_real(value), a.shortreal)
                    ));
                    continue;
                }
                out.push_str(&format!(
                    "    {}[{}] = {};\n",
                    a.c_name,
                    index,
                    coerce_two_state(
                        emit_const_for_vector(value, a.elem_width, a.signed)?,
                        a.two_state
                    )
                ));
            }
            IrInitStep::SetScalar { sig, value } => {
                let s = model.signal(*sig);
                let v = match s.ty {
                    IrType::Real { shortreal } => {
                        round_shortreal(emit_const_for_real(value), shortreal)
                    }
                    IrType::Packed {
                        width,
                        signed,
                        two_state,
                    } => coerce_two_state(emit_const_for_vector(value, width, signed)?, two_state),
                };
                out.push_str(&format!("    {} = {};\n", s.c_name, v));
            }
            IrInitStep::RegisterSampled(sig) => {
                let s = model.signal(*sig);
                out.push_str(&format!("    llg_sampled_register(&{});\n", s.c_name));
            }
            IrInitStep::WriteNet { group, slot, value } => {
                let g = model.net_group(*group);
                out.push_str(&format!(
                    "    llg_net_write(&{}, {}, {});\n",
                    g.c_name,
                    slot,
                    emit_const(value)
                ));
            }
            IrInitStep::Initialize(initialization) => {
                if initialization.phase() != crate::sim::ir::IrInitPhase::BeforeProcesses {
                    continue;
                }
                let value =
                    super::super::expressions::render_expr_impl(&ctx, initialization.value())?.code;
                match initialization.target() {
                    crate::sim::ir::IrInitTarget::Signal(signal) => {
                        let signal = model.signal(*signal);
                        let value = match signal.ty {
                            IrType::Real { shortreal } => round_shortreal(value, shortreal),
                            IrType::Packed { two_state, .. } => coerce_two_state(value, two_state),
                        };
                        out.push_str(&format!("    {} = {value};\n", signal.c_name));
                    }
                    crate::sim::ir::IrInitTarget::StaticLocal { function, name } => {
                        let local = model
                            .func(*function)
                            .locals
                            .iter()
                            .find(|local| local.c_name() == name)
                            .ok_or_else(|| {
                                format!(
                                    "declaration initializer references unknown static local `{name}`"
                                )
                            })?;
                        let value = if local.real {
                            round_shortreal(value, local.shortreal)
                        } else {
                            coerce_two_state(value, local.two_state)
                        };
                        out.push_str(&format!("    {name} = {value};\n"));
                    }
                }
            }
        }
    }
    for object in &model.objects {
        if let Some(value) = &object.initial {
            out.push_str(&format!(
                "    llg_string_move(&{}, {});\n",
                object.c_name,
                super::super::objects::string(&ctx, value)?
            ));
        }
    }
    if model.waveform {
        out.push_str(&format!(
            "    if (llg_wave_model_init({}ULL) != 0) return 1;\n",
            model.precision_fs
        ));
        for (index, sig) in model.signals.iter().enumerate() {
            let Some(hdl_name) = &sig.hdl_name else {
                continue;
            };
            if sig.omit && sig.net_alias.is_empty() {
                continue;
            }
            let registration = match sig.ty {
                IrType::Packed { width, .. } => format!(
                    "llg_wave_register_sv4({}, {}, {})",
                    c_string_literal(hdl_name),
                    if sig.net_alias.is_empty() {
                        format!("&{}", sig.c_name)
                    } else {
                        format!("&llg_net_alias_{index}.visible")
                    },
                    width
                ),
                IrType::Real { .. } => format!(
                    "llg_wave_register_real({}, &{})",
                    c_string_literal(hdl_name),
                    sig.c_name
                ),
            };
            out.push_str(&format!("    if ({registration} != 0) return 1;\n"));
        }
        for array in &model.arrays {
            for index in 0..array.total {
                let hdl_name = array
                    .waveform_element_name(index)
                    .unwrap_or_else(|| format!("{}[{index}]", array.hdl_name));
                let registration = if array.real {
                    format!(
                        "llg_wave_register_real({}, &{}[{}])",
                        c_string_literal(&hdl_name),
                        array.c_name,
                        index
                    )
                } else {
                    format!(
                        "llg_wave_register_sv4({}, &{}[{}], {})",
                        c_string_literal(&hdl_name),
                        array.c_name,
                        index,
                        array.elem_width
                    )
                };
                out.push_str(&format!("    if ({registration} != 0) return 1;\n"));
            }
        }
    }
    // Concurrent assertion predicates resolve every packed signal through the
    // runtime's immutable Preponed snapshot. Registering all active packed
    // storage keeps the callback contract simple and also covers nested
    // expression reads without a second dependency collector in the emitter.
    for signal in &model.signals {
        if !signal.omit && matches!(signal.ty, IrType::Packed { .. }) {
            out.push_str(&format!("    llg_sampled_register(&{});\n", signal.c_name));
        }
    }
    for (index, domain) in model.sampled_domains().iter().enumerate() {
        let clock = model.signal(domain.clock_signal).c_name();
        let edge = if domain.posedge {
            "LLG_EV_POSEDGE"
        } else {
            "LLG_EV_NEGEDGE"
        };
        let gate = if domain.gate.is_some() {
            sampled_domain_callback_name(index, "gate")
        } else {
            "NULL".to_owned()
        };
        out.push_str(&format!(
            "    if (!llg_sampled_domain_register({}ULL, &{}, {}, {}, {}, NULL)) return 1;\n",
            index,
            clock,
            edge,
            sampled_domain_callback_name(index, "value"),
            gate,
        ));
    }
    for (index, assertion) in model.assertions().iter().enumerate() {
        let clock = model.signal(assertion.clock_signal()).c_name();
        let disable = assertion
            .disable_signal()
            .map(|signal| format!("&{}", model.signal(signal).c_name()))
            .unwrap_or_else(|| "NULL".to_owned());
        let antecedent = assertion
            .antecedent()
            .map(|_| assertion_predicate_name(index, "antecedent"))
            .unwrap_or_else(|| "NULL".to_owned());
        let abort_condition = assertion
            .abort_condition()
            .map(|_| assertion_predicate_name(index, "abort"))
            .unwrap_or_else(|| "NULL".to_owned());
        let pass_action = assertion.pass_action().unwrap_or("NULL");
        let fail_action = assertion.fail_action().unwrap_or("NULL");
        let kind = match assertion.kind() {
            IrConcurrentAssertionKind::Assert => "LLG_ASSERTION_ASSERT",
            IrConcurrentAssertionKind::Assume => "LLG_ASSERTION_ASSUME",
            IrConcurrentAssertionKind::Cover => "LLG_ASSERTION_COVER",
            IrConcurrentAssertionKind::Expect => "LLG_ASSERTION_EXPECT",
        };
        let edge = if assertion.posedge() {
            "LLG_EV_POSEDGE"
        } else {
            "LLG_EV_NEGEDGE"
        };
        if assertion.consequent_sequence().is_some() {
            let antecedent = assertion
                .antecedent_sequence()
                .map(|_| format!("&{}", assertion_sequence_name(index, "antecedent")))
                .unwrap_or_else(|| "NULL".to_owned());
            let consequent = format!("&{}", assertion_sequence_name(index, "consequent"));
            if assertion.abort_condition().is_some() {
                out.push_str(&format!(
                    "    if (!llg_assertion_register_sequence_control(&{}, {}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}, {}, {}ULL, {}, {}, {})) return 1;\n",
                    clock,
                    edge,
                    disable,
                    antecedent,
                    consequent,
                    abort_condition,
                    pass_action,
                    fail_action,
                    kind,
                    assertion.overlapped() as u8,
                    assertion.abort_reject() as u8,
                    assertion.abort_sync() as u8,
                    assertion.identity(),
                    c_string_literal(assertion.label()),
                    c_string_literal(assertion.location()),
                    c_string_literal(assertion.scope()),
                ));
            } else {
                out.push_str(&format!(
                    "    if (!llg_assertion_register_sequence(&{}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}ULL, {}, {}, {})) return 1;\n",
                    clock,
                    edge,
                    disable,
                    antecedent,
                    consequent,
                    pass_action,
                    fail_action,
                    kind,
                    assertion.overlapped() as u8,
                    assertion.identity(),
                    c_string_literal(assertion.label()),
                    c_string_literal(assertion.location()),
                    c_string_literal(assertion.scope()),
                ));
            }
        } else {
            if assertion.consequent().is_none() {
                return Err(format!("assertion {index} has no consequent"));
            }
            let consequent = assertion_predicate_name(index, "consequent");
            if assertion.abort_condition().is_some() {
                out.push_str(&format!(
                    "    if (!llg_assertion_register_control(&{}, {}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}, {}, {}ULL, {}, {}, {})) return 1;\n",
                    clock,
                    edge,
                    disable,
                    antecedent,
                    consequent,
                    abort_condition,
                    pass_action,
                    fail_action,
                    kind,
                    assertion.overlapped() as u8,
                    assertion.abort_reject() as u8,
                    assertion.abort_sync() as u8,
                    assertion.identity(),
                    c_string_literal(assertion.label()),
                    c_string_literal(assertion.location()),
                    c_string_literal(assertion.scope()),
                ));
            } else {
                out.push_str(&format!(
                    "    if (!llg_assertion_register(&{}, {}, {}, {}, {}, {}, {}, NULL, {}, {}, {}ULL, {}, {}, {})) return 1;\n",
                    clock,
                    edge,
                    disable,
                    antecedent,
                    consequent,
                    pass_action,
                    fail_action,
                    kind,
                    assertion.overlapped() as u8,
                    assertion.identity(),
                    c_string_literal(assertion.label()),
                    c_string_literal(assertion.location()),
                    c_string_literal(assertion.scope()),
                ));
            }
        }
    }
    out.push_str(&format!(
        "    if (!llg_vpi_model_init({}, llg_vpi_objects, llg_vpi_object_count) || !llg_vpi_startup()) {{\n\
             \x20       llg_vpi_shutdown();\n\
             \x20       llg_rt_cleanup();\n\
             \x20       return 1;\n\
             \x20   }}\n",
        c_string_literal(model.design_name())
    ));
    for (index, call) in model.vpi_compile_calls.iter().enumerate() {
        out.push_str(&format!(
            "    if (!llg_vpi_compile_call_site({index}ULL, {}, llg_vpi_compile_args_{index}, {}, {}ULL)) {{\n\
             \x20       llg_vpi_shutdown();\n\
             \x20       llg_rt_cleanup();\n\
             \x20       return 1;\n\
             \x20   }}\n",
            c_string_literal(&call.name),
            call.args.len(),
            call.time_unit_fs,
        ));
    }
    out.push_str("    llg_vpi_start_simulation();\n");
    out.push_str(
        "    if (llg_vpi_failed()) {\n\
             \x20       llg_vpi_shutdown();\n\
             \x20       llg_rt_cleanup();\n\
             \x20       return 1;\n\
             \x20   }\n",
    );
    for (fname, label) in model.spawn_list() {
        let runtime_name = model
            .processes
            .iter()
            .find(|p| p.c_name == fname)
            .map(process_runtime_name)
            .unwrap_or_else(|| label.to_owned());
        let process = execution
            .processes()
            .iter()
            .find(|process| model.processes[process.semantic_process].c_name == fname);
        let region = process
            .map(|process| process.region)
            .unwrap_or(ScheduleRegion::Active);
        let semantic_process = process.map(|process| &model.processes[process.semantic_process]);
        if let Some(instance) = semantic_process.and_then(|process| process.program) {
            let is_initial =
                semantic_process.is_some_and(|process| process.kind() == IrProcessKind::Initial);
            out.push_str(&format!(
                "    llg_spawn_program_in_region({fname}, {}, {}, {instance}ULL, {});\n",
                c_string_literal(&runtime_name),
                region.runtime_symbol(),
                u8::from(is_initial)
            ));
        } else if region == ScheduleRegion::Active {
            out.push_str(&format!(
                "    llg_spawn({fname}, {});\n",
                c_string_literal(&runtime_name)
            ));
        } else {
            out.push_str(&format!(
                "    llg_spawn_in_region({fname}, {}, {});\n",
                c_string_literal(&runtime_name),
                region.runtime_symbol()
            ));
        }
    }
    // Capture scheduler exit time before user finals. Finals cannot advance
    // time, and registering this first also preserves the timestamp if a
    // user final calls `$finish` and stops the remaining final queue.
    if model.waveform {
        out.push_str(
            "    llg_spawn_final(llg_wave_capture_final_time, \
             \"llg.wave.capture_final_time\");\n",
        );
    }
    // Final blocks (`final begin … end`, SV 1800-2005 §10.7) register with
    // the runtime and run after the main scheduler loop exits.
    for fname in &model.final_spawns {
        let runtime_name = model
            .processes
            .iter()
            .find(|p| p.c_name == *fname)
            .map(process_runtime_name)
            .unwrap_or_default();
        out.push_str(&format!(
            "    llg_spawn_final({fname}, {});\n",
            c_string_literal(&runtime_name)
        ));
    }
    out.push_str("    llg_rt_run();\n");
    out.push_str("    if (!llg_rt_is_suspended()) {\n");
    if !model.final_spawns.is_empty() || model.waveform {
        out.push_str("        llg_rt_run_finals();\n");
    }
    out.push_str("    }\n");
    out.push_str("    llg_vpi_end_simulation();\n");
    for object in &model.objects {
        match object.ty {
            crate::sim::ir::IrObjectType::String => {
                out.push_str(&format!("    llg_string_destroy(&{});\n", object.c_name));
            }
            crate::sim::ir::IrObjectType::Process => {
                out.push_str(&format!("    llg_process_release({});\n", object.c_name));
                out.push_str(&format!("    {} = NULL;\n", object.c_name));
            }
            crate::sim::ir::IrObjectType::Chandle => {}
            crate::sim::ir::IrObjectType::Semaphore => {
                // Semaphore storage is owned by the runtime registry.  The
                // post-run pointer is only a stale model reference and must
                // not be released through the generic chandle path.
                out.push_str(&format!("    {} = NULL;\n", object.c_name));
            }
        }
    }
    for container in &model.containers {
        out.push_str(&super::super::containers::destroy(container));
    }
    if model.waveform {
        out.push_str(
            "    if (llg_rt_is_suspended()) {\n\
                     int llg_stop_wave_error = llg_wave_close(llg_time());\n\
                     llg_vpi_shutdown();\n\
                     llg_rt_cleanup();\n\
                     return llg_stop_wave_error == 0 ? 0 : 1;\n\
                 }\n",
        );
    } else {
        out.push_str(
            "    if (llg_rt_is_suspended()) {\n\
                     llg_vpi_shutdown();\n\
                     llg_rt_cleanup();\n\
                     return 0;\n\
                 }\n",
        );
    }
    if model.waveform {
        out.push_str(
            "    if (llg_rt_failed() || llg_vpi_failed()) { llg_vpi_shutdown(); return 1; }\n",
        );
        out.push_str("    llg_vpi_shutdown();\n");
        out.push_str("    return llg_wave_close(llg_wave_final_time);\n}\n");
    } else {
        out.push_str(
            "    if (llg_rt_failed() || llg_vpi_failed()) { llg_vpi_shutdown(); return 1; }\n",
        );
        out.push_str("    llg_vpi_shutdown();\n");
        out.push_str("    return 0;\n}\n");
    }
    Ok(out)
}
