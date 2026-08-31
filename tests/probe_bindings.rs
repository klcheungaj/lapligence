//! Throwaway probe: dump ref → vpiActual bindings for a small design.

use std::sync::Mutex;

use llg::core::compile;
use llg::ffi::vpi;
use llg::ffi::vpi::VpiHandle;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

fn in_temp_dir<R>(f: impl FnOnce() -> R) -> R {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_probe_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let result = f();
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn each_child<F: FnMut(VpiHandle)>(rel: std::os::raw::c_int, obj: VpiHandle, f: &mut F) {
    if let Some(it) = vpi::iterate(rel, obj) {
        for h in it {
            f(h);
        }
    } else if let Some(h) = vpi::handle(rel, obj) {
        f(h.raw());
    }
}

fn describe(h: VpiHandle) -> String {
    let t = vpi::obj_type(h);
    let name = vpi::obj_name(h);
    let full = vpi::obj_full_name(h);
    let file = vpi::get_str(vpi::vpiFile, h);
    let line = vpi::get(vpi::vpiLineNo, h);
    let col = vpi::get(vpi::vpiColumnNo, h);
    format!("type={t} name={name:?} full={full:?} file={file:?} line={line} col={col}")
}

const RELS: [std::os::raw::c_int; 8] = [
    vpi::vpiRhs,
    vpi::vpiLhs,
    vpi::vpiCondition,
    vpi::vpiOperand,
    vpi::vpiArgument,
    vpi::vpiStmt,
    vpi::vpiIndex,
    vpi::vpiExpr,
];

fn walk_expr(h: VpiHandle, depth: usize) {
    if h.is_null() || depth > 24 {
        return;
    }
    let t = vpi::obj_type(h);
    if matches!(
        t,
        vpi::uhdmref_obj | vpi::uhdmref_var | vpi::vpiRefObj | vpi::vpiVarSelect
    ) {
        println!("  REF  {}", describe(h));
        if let Some(actual) = vpi::handle(vpi::vpiActual, h) {
            println!("       -> actual {}", describe(actual.raw()));
        } else {
            println!("       -> actual NULL");
        }
        return;
    }
    for rel in RELS {
        each_child(rel, h, &mut |child| walk_expr(child, depth + 1));
    }
}

fn visit_inst(mi: VpiHandle) {
    println!("INST {}", describe(mi));
    // Ports with high/low conns.
    for port in vpi::iterate(vpi::vpiPort, mi).into_iter().flatten() {
        println!("  PORT {}", describe(port));
        if let Some(high) = vpi::handle(vpi::vpiHighConn, port) {
            println!("    HIGH {}", describe(high.raw()));
            walk_expr(high.raw(), 0);
        } else {
            println!("    HIGH None");
        }
        if let Some(low) = vpi::handle(vpi::vpiLowConn, port) {
            println!("    LOW {}", describe(low.raw()));
        } else {
            println!("    LOW None");
        }
    }
    for assign in vpi::iterate(vpi::vpiContAssign, mi).into_iter().flatten() {
        println!("  CONT_ASSIGN {}", describe(assign));
        if let Some(lhs) = vpi::handle(vpi::vpiLhs, assign) {
            println!("    LHS-tree:");
            walk_expr(lhs.raw(), 0);
        }
        if let Some(rhs) = vpi::handle(vpi::vpiRhs, assign) {
            println!("    RHS-tree:");
            walk_expr(rhs.raw(), 0);
        }
    }
    for proc in vpi::iterate(vpi::vpiProcess, mi).into_iter().flatten() {
        println!("  PROCESS {}", describe(proc));
        each_child(vpi::vpiStmt, proc, &mut |st| walk_expr(st, 0));
    }
    for child in vpi::iterate(vpi::vpiModule, mi).into_iter().flatten() {
        visit_inst(child);
    }
}

fn t_of(h: VpiHandle) -> std::os::raw::c_int {
    vpi::obj_type(h)
}

#[test]
fn probe_bindings() {
    in_temp_dir(|| {
        let dir = std::env::temp_dir().join(format!("llg_probe_src_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fa = dir.join("fileA.sv");
        std::fs::write(
            &fa,
            "module m_a;\n  logic clk;\n  logic oa;\n  assign oa = ~clk;\nendmodule\n",
        )
        .unwrap();
        let fb = dir.join("fileB.sv");
        std::fs::write(
            &fb,
            "module m_b;\n  logic clk;\n  logic ob;\n  always_comb ob = ~clk;\nendmodule\n",
        )
        .unwrap();
        let tb = dir.join("tb.sv");
        std::fs::write(
            &tb,
            "module tb;\n  logic wa;\n  logic wb;\n  logic t;\n  m_a u_a(.clk(wa));\n  m_b u_b(.clk(wb));\n  assign t = wa ^ wb;\nendmodule\n",
        )
        .unwrap();

        let opts = compile::CompileOpts {
            files: vec![
                fa.to_string_lossy().into_owned(),
                fb.to_string_lossy().into_owned(),
                tb.to_string_lossy().into_owned(),
            ],
            top: None,
            ..Default::default()
        };
        let out = compile::compile(&opts).expect("compile");
        assert!(out.ok(), "diagnostics: {:?}", out.diagnostics);
        let design = out.uhdm_design().expect("design handle");

        for top in vpi::iterate(vpi::uhdmtopModules, design)
            .into_iter()
            .flatten()
        {
            visit_inst(top);
        }

        // Also check what uhdmallModules yields (what tokens.rs walks).
        println!("\n== uhdmallModules ==");
        for m in vpi::iterate(vpi::uhdmallModules, design)
            .into_iter()
            .flatten()
        {
            println!("ALLMOD {}", describe(m));
            for proc in vpi::iterate(vpi::vpiProcess, m).into_iter().flatten() {
                println!("  DEFPROCESS {}", describe(proc));
                each_child(vpi::vpiStmt, proc, &mut |st| walk_expr(st, 0));
            }
            for port in vpi::iterate(vpi::vpiPort, m).into_iter().flatten() {
                println!("  DEFPORT {}", describe(port));
                if let Some(low) = vpi::handle(vpi::vpiLowConn, port) {
                    println!("    LOW {}", describe(low.raw()));
                } else {
                    println!("    LOW None");
                }
            }
        }

        // And the direct uhdmref_obj children of scopes (walk_scope's last loop).
        println!("\n== scope-level uhdmref_obj iteration on tops ==");
        for top in vpi::iterate(vpi::uhdmtopModules, design)
            .into_iter()
            .flatten()
        {
            for r in vpi::iterate(vpi::uhdmref_obj, top).into_iter().flatten() {
                println!("SCOPEREF {}", describe(r));
                if let Some(a) = vpi::handle(vpi::vpiActual, r) {
                    println!("   -> {}", describe(a.raw()));
                }
            }
        }
    });
}
