//! Embeddable start/advance/close API. Suspension does not destroy live owners.
use super::*;
use crate::sim::execution::ScheduleRegion;

pub(in crate::sim::emit_c) fn main(execution: &ExecutionModel) -> Result<String, String> {
    let model = execution.ir();
    let mut out = String::from(
        "/* start: 0=ready, 1=error; advance: 0=complete, 1=error, 2=suspended.\n\
         * A suspended model retains all owners until another advance or close.\n\
         * Define LLG_MODEL_NO_MAIN to drive these entry points from a host. */\n\
         static int llg_model_live, llg_model_done, llg_model_status;\n\
         int llg_model_close(void);\n");
    if model.waveform { out.push_str("static int llg_model_wave_live;\n"); }
    out.push_str(&format!("int llg_model_start(int argc, char** argv) {{\n    if (llg_model_live) return 1;\n    llg_model_live = 1;\n    llg_model_done = llg_model_status = 0;\n    llg_rt_init_with_args_precision_and_stack(argc, argv, {}ULL, LLG_MODEL_STACK_VALUES);\n    if (llg_rt_failed()) goto start_failed;\n    llg_model_storage_defaults();\n    llg_model_initializers();\n    if (llg_rt_failed()) goto start_failed;\n", model.precision_fs));
    // Mark otherwise unused generated function definitions as intentional.
    for function in &model.funcs { out.push_str(&format!("    (void){};\n", function.c_name)); }
    if model.waveform {
        out.push_str(&format!("    llg_wave_final_time = 0;\n    llg_model_wave_live = 1;\n    if (llg_wave_model_init({}ULL) != 0) goto start_failed;\n", model.precision_fs));
        for signal in &model.signals {
            if signal.omit && signal.net_driver.is_none() { continue; }
            let Some(name) = &signal.hdl_name else { continue; };
            let call = match signal.ty {
                IrType::Packed { width, .. } => format!("llg_wave_register_sv4({}, &{}, {width})", c_string_literal(name), signal.c_name),
                IrType::Real { .. } => format!("llg_wave_register_real({}, &{})", c_string_literal(name), signal.c_name),
            };
            out.push_str(&format!("    if ({call} != 0) goto start_failed;\n"));
        }
        for array in &model.arrays {
            for index in 0..array.total {
                let name = array.waveform_element_name(index).ok_or_else(|| "invalid waveform array index".to_owned())?;
                let call = if array.real { format!("llg_wave_register_real({}, &{}[{index}])", c_string_literal(&name), array.c_name) }
                    else { format!("llg_wave_register_sv4({}, &{}[{index}], {})", c_string_literal(&name), array.c_name, array.elem_width) };
                out.push_str(&format!("    if ({call} != 0) goto start_failed;\n"));
            }
        }
    }
    out.push_str(&format!("    if (!llg_vpi_model_init({}, llg_vpi_objects, llg_vpi_object_count) || !llg_vpi_startup()) goto start_failed;\n    llg_vpi_start_simulation();\n    if (llg_vpi_failed()) goto start_failed;\n", c_string_literal(model.design_name())));
    for (name, fallback_label) in model.spawn_list() {
        let process = execution.processes().iter().find(|item| model.processes[item.semantic_process].c_name == name);
        let semantic = process.map(|item| &model.processes[item.semantic_process]);
        let region = process.map(|item| item.region).unwrap_or(ScheduleRegion::Active);
        let label = semantic.map(|item| item.label()).unwrap_or(fallback_label);
        if let Some(instance) = semantic.and_then(|item| item.program) {
            let initial = semantic.is_some_and(|item| item.kind() == IrProcessKind::Initial);
            out.push_str(&format!("    llg_spawn_program_in_region({name}, {}, {}, {instance}ULL, {});\n", c_string_literal(label), region.runtime_symbol(), u8::from(initial)));
        } else { out.push_str(&format!("    llg_spawn_in_region({name}, {}, {});\n", c_string_literal(label), region.runtime_symbol())); }
    }
    if model.waveform { out.push_str("    llg_spawn_final(llg_wave_capture_final_time, \"llg.wave.capture_final_time\");\n"); }
    for name in &model.final_spawns {
        let label = model.processes.iter().find(|p| &p.c_name == name).map(|p| p.label()).unwrap_or(name);
        out.push_str(&format!("    llg_spawn_final({name}, {});\n", c_string_literal(label)));
    }
    out.push_str("    return 0;\nstart_failed:\n    (void)llg_model_close();\n    return 1;\n}\n\n");
    out.push_str("int llg_model_advance(void) {\n    if (!llg_model_live) return 1;\n    if (llg_model_done) return llg_model_status;\n    if (llg_rt_is_suspended() && !llg_rt_resume()) return 1;\n    llg_rt_run();\n    if (llg_rt_is_suspended()) return 2;\n");
    if model.waveform || !model.final_spawns.is_empty() { out.push_str("    llg_rt_run_finals();\n"); }
    out.push_str("    llg_vpi_end_simulation();\n    llg_model_done = 1;\n    llg_model_status = (llg_rt_failed() || llg_vpi_failed()) ? 1 : 0;\n    return llg_model_status;\n}\n\n");
    out.push_str("int llg_model_close(void) {\n    int status = 0;\n    if (!llg_model_live) return 0;\n");
    if model.waveform {
        out.push_str("    if (llg_model_wave_live) {\n        status = llg_wave_close(llg_model_done ? llg_wave_final_time : llg_time()) != 0;\n        llg_model_wave_live = 0;\n    }\n");
    }
    out.push_str("    llg_vpi_shutdown();\n    llg_rt_cleanup();\n    llg_model_storage_destroy();\n    llg_model_live = llg_model_done = llg_model_status = 0;\n    return status;\n}\n\n#ifndef LLG_MODEL_NO_MAIN\nint main(int argc, char** argv) {\n    int status = llg_model_start(argc, argv);\n    if (status == 0) {\n        status = llg_model_advance();\n        if (status == 2) status = 0; /* CLI exit-policy stop, not a model error. */\n    }\n    if (llg_model_close() != 0) status = 1;\n    return status;\n}\n#endif\n");
    Ok(out)
}
