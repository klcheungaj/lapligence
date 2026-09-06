//! Whole-model assembly, storage declarations, processes, and initialization.

use super::constants::{
    c_string_literal, emit_all_known_init, emit_all_x_init, emit_all_z_init, emit_const,
    emit_const_for_real, emit_const_for_vector, round_shortreal,
};
use super::context::RCtx;
use super::expressions::{coerce_two_state, packed_default};
use super::statements::{
    render_pre_fn_impl as render_pre_fn, render_stmt_impl as render_stmt, wait_any_text,
};
use super::EmitError;
use crate::sim::ir::{IrFunc, IrModel, IrNetKind, IrType};
use std::collections::HashSet;

// ── Model rendering ───────────────────────────────────────────────────────────

/// Render the complete `model.c` for a lowered (and optimized) IR model:
/// the header comment the driver parses, signal/net/array storage, function
/// prototypes and bodies, process functions, and `main()`.
pub fn render(model: &IrModel) -> Result<String, EmitError> {
    let capacity = model
        .packed_capacity()
        .map_err(EmitError::InvalidIr)?
        .max(64);
    if capacity >= u128::from(super::LLG_WIDTH_LIMIT) {
        return Err(EmitError::new(format!(
            "packed width {capacity} reaches the C runtime exclusive limit {}",
            super::LLG_WIDTH_LIMIT
        )));
    }
    render_model(model, capacity as u32).map_err(EmitError::new)
}

fn render_model(model: &IrModel, capacity: u32) -> Result<String, String> {
    let mut out = format!(
        "// llg-generated C11 model for design `{}`\n",
        model.design_name
    );
    out.push_str(&format!("#define LLG_MODEL_MAX_WIDTH {capacity}\n"));
    out.push_str(&format!(
        "#define LLG_MODEL_STACK_VALUES {}\n",
        super::stack::stack_value_slots(model)?
    ));
    if model.waveform {
        out.push_str("#define LLG_WAVEFORM 1\n");
    }
    out.push_str("#include \"llg_rt.h\"\n");
    if model.waveform {
        out.push_str("#include \"llg_wave.h\"\n");
    }
    out.push_str(
        "\n#include <stdio.h>\n#include <math.h>\n\n\
         /* signals start all-X; driven by processes and link processes */\n",
    );
    render_signal_decls(model, &mut out);
    out.push('\n');
    // Arrays start all-X; elements are filled in `main()` (a function call
    // is not a valid static initializer).
    for a in &model.arrays {
        out.push_str(&format!("sv4_t {}[{}];\n", a.c_name, a.total));
    }
    out.push('\n');
    if model.waveform {
        out.push_str(
            "static uint64_t llg_wave_final_time;\n\
             static void llg_wave_capture_final_time(llg_proc_t* self) {\n\
             \x20   llg_wave_final_time = llg_time();\n\
             \x20   llg_proc_done(self);\n\
             \x20   return;\n\
             }\n\n",
        );
    }
    // Functions/tasks become static C functions (prototypes first so bodies
    // may call each other regardless of declaration order), emitted before
    // any process code references them.
    for f in &model.funcs {
        out.push_str(&func_prototype(f));
    }
    let ctx = RCtx { model, func: None };
    for f in &model.funcs {
        let fctx = RCtx {
            model,
            func: Some(f),
        };
        for pre in &f.pre_fns {
            out.push_str(&render_pre_fn(&ctx, pre)?);
        }
        out.push_str(&render_func_body(&fctx, f)?);
    }
    // Three passes lower comb drivers, links, then always/initial processes,
    // so every comb process, link, and process runs at t=0 in that order;
    // push order equals spawn order.
    for p in &model.processes {
        for pre in &p.pre_fns {
            out.push_str(&render_pre_fn(&ctx, pre)?);
        }
        out.push_str(&render_process_fn(&ctx, p)?);
    }
    out.push_str(&render_main(model)?);
    Ok(out)
}

/// Signal globals plus collapsed inout-net group storage.
fn render_signal_decls(model: &IrModel, out: &mut String) {
    let mut emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for sig in &model.signals {
        if sig.net_driver.is_some() || sig.omit || !emitted.insert(sig.c_name.as_str()) {
            // Net-group members: storage is emitted with its group; omitted
            // signals are pruned by `unused_storage`.
            continue;
        }
        match sig.ty {
            IrType::Real { .. } => out.push_str(&format!("double {} = 0.0;\n", sig.c_name)),
            IrType::Packed {
                width,
                signed,
                two_state,
            } => {
                // `SV4_X` clamps to 64 bits, so init wide signals with an
                // all-X brace initializer mirroring the runtime `sv4_x`.
                let init = if two_state {
                    emit_all_known_init(width, signed, false)
                } else if width <= 64 {
                    format!(
                        "SV4_INIT(0, LLG_MASK({width}), 0, {width}, {})",
                        signed as u8
                    )
                } else {
                    emit_all_x_init(width, signed)
                };
                out.push_str(&format!("sv4_t {} = {init};\n", sig.c_name));
            }
        }
    }
    let mut groups_emitted: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for g in &model.net_groups {
        if !groups_emitted.insert(g.c_name.as_str()) {
            continue;
        }
        let driver_init = if g.width <= 64 {
            format!("SV4_Z({})", g.width)
        } else {
            emit_all_z_init(g.width)
        };
        let resolved_init = match g.kind {
            IrNetKind::Tri0 | IrNetKind::Supply0 => emit_all_known_init(g.width, g.signed, false),
            IrNetKind::Tri1 | IrNetKind::Supply1 => emit_all_known_init(g.width, g.signed, true),
            IrNetKind::Wire | IrNetKind::Wand | IrNetKind::Wor => driver_init.clone(),
        };
        let mut driver_ptrs = Vec::with_capacity(g.n_drivers);
        for slot in 0..g.n_drivers {
            let cell = format!("{}_d{}", g.c_name, slot);
            out.push_str(&format!("sv4_t {cell} = {driver_init};\n"));
            driver_ptrs.push(format!("&{cell}"));
        }
        out.push_str(&format!(
            "static llg_net_t {} = {{ {resolved_init}, {}, {}, {}, {}, {{ {} }} }};\n",
            g.c_name,
            g.width,
            g.signed as u8,
            g.kind.c_value(),
            g.n_drivers,
            driver_ptrs.join(", ")
        ));
    }
    // Named events: one waiter-table global per declared event (never pruned
    // — events are wakeup channels, not storage).
    for ev in &model.events {
        out.push_str(&format!(
            "static llg_event_t {} = {{{{ 0 }}, 0 }};\n",
            ev.c_name
        ));
    }
}

/// The C parameter list of a lowered function: outputs first (`o{formal
/// idx}`), then inputs (`a{formal idx}`), then the recursion depth.
fn func_params(f: &IrFunc) -> String {
    let mut params = Vec::new();
    for (idx, form) in f.formals.iter().enumerate() {
        if form.is_out {
            params.push(format!("sv4_t* o{idx}"));
        }
    }
    for (idx, form) in f.formals.iter().enumerate() {
        if !form.is_out {
            params.push(format!("sv4_t a{idx}"));
        }
    }
    params.push("int depth".to_string());
    params.join(", ")
}

fn func_prototype(f: &IrFunc) -> String {
    let ret_t = if f.ret.is_some() { "sv4_t" } else { "void" };
    format!("static {ret_t} {}({});\n", f.c_name, func_params(f))
}

/// The recursion depth guard at the top of every emitted function; it returns
/// the return type's default value (or nothing) after reporting excessive nesting.
const LLG_MAX_FUNC_DEPTH: u32 = 256;

fn render_func_body(ctx: &RCtx<'_>, f: &IrFunc) -> Result<String, String> {
    let ret_t = if f.ret.is_some() { "sv4_t" } else { "void" };
    let mut out = format!("static {ret_t} {}({}) {{\n", f.c_name, func_params(f));
    // The all-X return value used by the recursion guard.
    let ret_clause = if f.ret.is_some() {
        format!("return {};", f.ret_x())
    } else {
        "return;".to_string()
    };
    out.push_str(&format!(
        "    if (depth >= {LLG_MAX_FUNC_DEPTH}) {{\n        fprintf(stderr, \
         \"llg: recursion limit exceeded in %s\\n\", \"{}\");\n        \
         {ret_clause}\n    }}\n",
        f.c_name
    ));
    if let Some(IrType::Packed {
        width,
        signed,
        two_state,
    }) = f.ret
    {
        // Function-name return variable → `_ret` local.
        out.push_str(&format!(
            "    sv4_t _ret = {};\n",
            packed_default(width, signed, two_state)
        ));
    }
    for l in &f.locals {
        out.push_str(&format!(
            "    sv4_t {} = {};\n",
            l.c_name,
            packed_default(l.width, l.signed, l.two_state)
        ));
    }
    out.push_str(&block_stmts_of(ctx, &f.body)?);
    out.push_str("    ");
    if f.ret.is_some() {
        out.push_str("return _ret;\n");
    }
    out.push_str("}\n\n");
    Ok(out)
}

fn block_stmts_of(ctx: &RCtx<'_>, stmts: &[crate::sim::ir::IrStmt]) -> Result<String, String> {
    let mut out = String::new();
    for s in stmts {
        out.push_str(&render_stmt(ctx, s)?);
    }
    Ok(out)
}

fn render_process_fn(ctx: &RCtx<'_>, p: &crate::sim::ir::IrProcess) -> Result<String, String> {
    use crate::sim::ir::{IrShape, IrStmt};
    let mut out = format!(
        "static void {}(llg_proc_t* self) {{\n    (void)self;\n",
        p.c_name
    );
    let body = block_stmts_of(ctx, &p.body)?;
    match &p.shape {
        IrShape::RunOnce => {
            out.push_str(&body);
            out.push_str("    llg_proc_done(self);\n    return;\n");
        }
        IrShape::Loop => {
            // always / always_comb / always_ff bodies contain their own waits.
            out.push_str("for (;;) {\n");
            out.push_str(&body);
            out.push_str("    }\n");
        }
        IrShape::SensLoop { reads } => {
            out.push_str(&body);
            out.push_str("    for (;;) {\n");
            out.push_str(&wait_any_text(reads));
            // The in-loop copy indents one level deeper than the first
            // evaluation (plain drivers and begin blocks alike).
            // Control-flow labels in the body (break/continue/disable
            // targets) would be DEFINED twice — once per copy — so the
            // copy is relabeled with a suffix.  Lowering guarantees every
            // `goto` targets a label inside the same body tree, so the
            // rename stays internally consistent.
            let renamed = rename_stmt_labels(&p.body);
            for s in &renamed {
                let text = render_stmt(ctx, s)?;
                out.push_str("    ");
                out.push_str(&text);
            }
            out.push_str("    }\n");
        }
    }
    let _ = IrStmt::Nop;
    out.push_str("}\n\n");
    Ok(out)
}

/// Suffix appended to control-flow labels in the re-evaluation copy of a
/// combinational (`SensLoop`) process body.
const LOOP_COPY_SUFFIX: &str = "_r";

/// Collect every label DEFINED in a statement tree.
fn collect_label_names(stmts: &[crate::sim::ir::IrStmt], out: &mut HashSet<String>) {
    use crate::sim::ir::IrStmt;
    for s in stmts {
        match s {
            IrStmt::Label(l) => {
                out.insert(l.clone());
            }
            IrStmt::Block(b) | IrStmt::Forever { body: b } => collect_label_names(b, out),
            IrStmt::If { then_, els, .. } => {
                collect_label_names(then_, out);
                if let Some(els) = els {
                    collect_label_names(els, out);
                }
            }
            IrStmt::While { body: b, .. } | IrStmt::Repeat { body: b, .. } => {
                collect_label_names(b, out)
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                collect_label_names(init, out);
                collect_label_names(incr, out);
                collect_label_names(body, out);
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    collect_label_names(&item.body, out);
                }
            }
            IrStmt::WaitCond { body: b, .. } => collect_label_names(b, out),
            _ => {}
        }
    }
}

/// Rewrite `Label`/`Goto` strings in place for the names defined in
/// `names` (a goto can only target a label defined in the same tree).
fn rename_labels_in(stmts: &mut [crate::sim::ir::IrStmt], names: &HashSet<String>) {
    use crate::sim::ir::IrStmt;
    for s in stmts {
        match s {
            IrStmt::Label(l) | IrStmt::Goto(l) => {
                if names.contains(l.as_str()) {
                    l.push_str(LOOP_COPY_SUFFIX);
                }
            }
            IrStmt::Block(b) | IrStmt::Forever { body: b } => rename_labels_in(b, names),
            IrStmt::If { then_, els, .. } => {
                rename_labels_in(then_, names);
                if let Some(els) = els {
                    rename_labels_in(els, names);
                }
            }
            IrStmt::While { body: b, .. } | IrStmt::Repeat { body: b, .. } => {
                rename_labels_in(b, names)
            }
            IrStmt::For {
                init, incr, body, ..
            } => {
                rename_labels_in(init, names);
                rename_labels_in(incr, names);
                rename_labels_in(body, names);
            }
            IrStmt::Case { items, .. } => {
                for item in items {
                    rename_labels_in(&mut item.body, names);
                }
            }
            IrStmt::WaitCond { body: b, .. } => rename_labels_in(b, names),
            _ => {}
        }
    }
}

/// A relabeled clone of a combinational process body for its re-evaluation
/// copy (see the `SensLoop` renderer).  Bodies without labels are returned
/// unchanged.
fn rename_stmt_labels(stmts: &[crate::sim::ir::IrStmt]) -> Vec<crate::sim::ir::IrStmt> {
    let mut names = HashSet::new();
    collect_label_names(stmts, &mut names);
    let mut out = stmts.to_vec();
    if !names.is_empty() {
        rename_labels_in(&mut out, &names);
    }
    out
}

fn render_main(model: &IrModel) -> Result<String, String> {
    use crate::sim::ir::IrInitStep;
    let mut out = String::from("int main(void) {\n    llg_rt_init();\n");
    for step in &model.init_steps {
        match step {
            IrInitStep::FillArrayX(arr) => {
                let a = model.array(*arr);
                out.push_str(&format!(
                    "    {{ for (uint64_t _i = 0; _i < {}; _i++) {}[_i] = {}; }}\n",
                    a.total,
                    a.c_name,
                    packed_default(a.elem_width, a.signed, a.two_state)
                ));
            }
            IrInitStep::SetArrayElem { arr, index, value } => {
                let a = model.array(*arr);
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
            IrInitStep::WriteNet { group, slot, value } => {
                let g = model.net_group(*group);
                out.push_str(&format!(
                    "    llg_net_write(&{}, {}, {});\n",
                    g.c_name,
                    slot,
                    emit_const(value)
                ));
            }
        }
    }
    if model.waveform {
        out.push_str(&format!(
            "    if (llg_wave_model_init({}ULL) != 0) return 1;\n",
            model.precision_ps
        ));
        for sig in &model.signals {
            let Some(hdl_name) = &sig.hdl_name else {
                continue;
            };
            if sig.omit {
                continue;
            }
            let registration = match sig.ty {
                IrType::Packed { width, .. } => format!(
                    "llg_wave_register_sv4({}, &{}, {})",
                    c_string_literal(hdl_name),
                    sig.c_name,
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
                let hdl_name = format!("{}[{index}]", array.hdl_name);
                out.push_str(&format!(
                    "    if (llg_wave_register_sv4({}, &{}[{}], {}) != 0) return 1;\n",
                    c_string_literal(&hdl_name),
                    array.c_name,
                    index,
                    array.elem_width
                ));
            }
        }
    }
    for (fname, label) in model.spawn_list() {
        out.push_str(&format!("    llg_spawn({fname}, \"{label}\");\n"));
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
        let label = model
            .processes
            .iter()
            .find(|p| p.c_name == *fname)
            .map(|p| p.label.clone())
            .unwrap_or_default();
        out.push_str(&format!("    llg_spawn_final({fname}, \"{label}\");\n"));
    }
    out.push_str("    llg_rt_run();\n");
    if !model.final_spawns.is_empty() || model.waveform {
        out.push_str("    llg_rt_run_finals();\n");
    }
    if model.waveform {
        out.push_str("    return llg_wave_close(llg_wave_final_time);\n}\n");
    } else {
        out.push_str("    return 0;\n}\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ir::{
        IrArray, IrConst, IrExpr, IrExprKind, IrProcess, IrShape, IrSignal, IrStmt,
    };

    fn packed_const(value: u64) -> IrExpr {
        IrExpr::new(
            IrExprKind::Const(IrConst {
                bits: vec![value],
                x: vec![0],
                z: vec![0],
                width: 64,
                signed: false,
                real: None,
                fill: None,
            }),
            64,
            false,
            None,
        )
    }

    #[test]
    fn non_waveform_model_has_no_waveform_integration() {
        let c = render(&IrModel::new("plain".to_string(), 1).unwrap()).unwrap();

        assert!(!c.contains("#define LLG_WAVEFORM 1"));
        assert!(!c.contains("llg_wave.h"));
        assert!(!c.contains("llg_wave_model_init"));
        assert!(c.ends_with("    return 0;\n}\n"));
    }

    #[test]
    fn waveform_model_emits_controls_hierarchy_and_final_time_close() {
        let controls = vec![
            IrStmt::WaveFile("trace\\\"name.vcd".to_string()),
            IrStmt::WaveDumpVars,
            IrStmt::WaveOn,
            IrStmt::WaveOff,
            IrStmt::WaveDumpAll,
            IrStmt::WaveFlush,
            IrStmt::WaveLimit(packed_const(4096)),
        ];
        let mut model = IrModel::new("top".to_string(), 10).unwrap();
        model.waveform = true;
        model.signals = vec![
            IrSignal {
                c_name: "G_top_g_0__value".to_string(),
                hdl_name: Some("top\u{1f}g[0]\u{1f}value".to_string()),
                ty: IrType::Packed {
                    width: 12,
                    signed: false,
                    two_state: false,
                },
                net_driver: None,
                omit: false,
            },
            IrSignal {
                c_name: "g_net_0.resolved".to_string(),
                hdl_name: Some("top\u{1f}alias".to_string()),
                ty: IrType::Packed {
                    width: 1,
                    signed: false,
                    two_state: false,
                },
                net_driver: Some((0, 0)),
                omit: false,
            },
            IrSignal {
                c_name: "D_top_r".to_string(),
                hdl_name: Some("top\u{1f}r".to_string()),
                ty: IrType::Real { shortreal: false },
                net_driver: None,
                omit: false,
            },
            IrSignal {
                c_name: "G_top_pca$0_en".to_string(),
                hdl_name: None,
                ty: IrType::Packed {
                    width: 1,
                    signed: false,
                    two_state: false,
                },
                net_driver: None,
                omit: false,
            },
        ];
        model.net_groups = vec![crate::sim::ir::IrNetGroup {
            c_name: "g_net_0".to_string(),
            width: 1,
            signed: false,
            kind: crate::sim::ir::IrNetKind::Wire,
            n_drivers: 1,
        }];
        model.arrays = vec![IrArray {
            c_name: "G_top_mem".to_string(),
            hdl_name: "top\u{1f}mem".to_string(),
            elem_width: 8,
            signed: false,
            two_state: false,
            dims: vec![(1, 0)],
            total: 2,
        }];
        model.processes = vec![IrProcess {
            c_name: "p_top_initial_0".to_string(),
            label: "top.initial".to_string(),
            shape: IrShape::RunOnce,
            pre_fns: Vec::new(),
            body: controls,
        }];
        model.spawns = vec!["p_top_initial_0".to_string()];

        let c = render(&model).unwrap();

        assert_eq!(c.matches("#define LLG_WAVEFORM 1").count(), 1);
        assert!(c.contains("#include \"llg_wave.h\""));
        assert!(c.contains("llg_wave_file(\"trace\\\\\\\"name.vcd\", llg_time());"));
        assert!(c.contains("llg_wave_dumpvars(llg_time());"));
        assert!(c.contains("llg_wave_on(llg_time());"));
        assert!(c.contains("llg_wave_off(llg_time());"));
        assert!(c.contains("llg_wave_dumpall(llg_time());"));
        assert!(c.contains("llg_wave_flush(llg_time());"));
        assert!(c.contains("llg_wave_limit(sv4_to_u64("));
        assert!(c.contains("llg_wave_model_init(10ULL)"));
        assert!(
            c.contains("llg_wave_register_sv4(\"top\\037g[0]\\037value\", &G_top_g_0__value, 12)")
        );
        assert!(c.contains("llg_wave_register_sv4(\"top\\037alias\", &g_net_0.resolved, 1)"));
        assert!(c.contains("llg_wave_register_real(\"top\\037r\", &D_top_r)"));
        assert!(!c.contains("llg_wave_register_sv4(\"G_top_pca$0_en"));
        assert!(c.contains("llg_wave_register_sv4(\"top\\037mem[0]\", &G_top_mem[0], 8)"));
        assert!(c.contains("llg_wave_register_sv4(\"top\\037mem[1]\", &G_top_mem[1], 8)"));
        assert!(c.contains("llg_spawn_final(llg_wave_capture_final_time"));
        assert!(c.contains("return llg_wave_close(llg_wave_final_time);"));
    }
}
