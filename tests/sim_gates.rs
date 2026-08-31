//! End-to-end simulator tests for structural gate primitives
//! (IEEE 1364-1995 ch. 7 §7.1–7.2): n-input gates (`and`/`or`/`nand`/`nor`/
//! `xor`/`xnor`), `buf`/`not`, enable gates (`bufif0/1`, `notif0/1`),
//! `pullup`/`pulldown`, optional gate delays, and the documented v1 rejects
//! (UDP instances, switch/transistor primitives, gate arrays, strengths,
//! width mismatches).
//!
//! Expected traces are hand-computed from the LRM gate tables and the
//! runtime's `sv4_*` X/Z semantics: Z behaves as X in every expression
//! context — LRM 11.4.5 — a disabled enable gate drives Z, and an ENABLED
//! enable gate turns a data Z into X like buf/not (LRM 1364-1995 §7.4
//! Table 7-5).
//!
//! Surelog writes `slpp_all/` into the process working directory, so each
//! test uses a fresh temp directory and the process-wide mutex serializes
//! compile/codegen runs with the other simulator integration tests.

use std::process::Command;
use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Compile, codegen, build the generated C model, run it; return stdout.
fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("llg_sim_gates_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir: {e}"))?;
    let src = dir.join("tb.sv");
    std::fs::write(&src, sv).map_err(|e| format!("write source: {e}"))?;

    let orig_cwd = std::env::current_dir().map_err(|e| format!("current dir: {e}"))?;
    std::env::set_current_dir(&dir).map_err(|e| format!("chdir: {e}"))?;
    let result = (|| -> Result<String, String> {
        let out = compile::compile(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        let generated = sim::codegen::generate(design).map_err(|e| format!("codegen: {e}"))?;
        let exe = sim::build::build_model_cmake(&dir, &[("model.c", generated.model_c.as_str())])
            .map_err(|e| format!("cmake: {e}"))?;
        let output = Command::new(&exe)
            .output()
            .map_err(|e| format!("run: {e}"))?;
        if !output.status.success() {
            return Err(format!(
                "sim exited with {:?}, stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    })();

    std::env::set_current_dir(&orig_cwd).map_err(|e| format!("restore cwd: {e}"))?;
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Compile and codegen without building or running; preserves the codegen
/// error for rejection cases.
fn codegen_error(sv: &str, tag: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("llg_sim_gates_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir: {e}"))?;
    let src = dir.join("tb.sv");
    std::fs::write(&src, sv).map_err(|e| format!("write source: {e}"))?;

    let orig_cwd = std::env::current_dir().map_err(|e| format!("current dir: {e}"))?;
    std::env::set_current_dir(&dir).map_err(|e| format!("chdir: {e}"))?;
    let result = (|| -> Result<String, String> {
        let out = compile::compile(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        match sim::codegen::generate(design) {
            Ok(_) => Err("codegen unexpectedly succeeded".to_string()),
            Err(e) => Ok(e),
        }
    })();

    std::env::set_current_dir(&orig_cwd).map_err(|e| format!("restore cwd: {e}"))?;
    let _ = std::fs::remove_dir_all(&dir);
    result
}

// ── Basic two-input truth tables incl. X/Z propagation ──────────────────────

// Hand-computed per LRM tables and sv4 semantics (0 dominates AND, 1
// dominates OR, x^y=x, Z behaves as X in ops):
//
//   a b | and or nand nor xor xnor not(buf of a)
//   0 0 |  0   0   1    1   0    1     1
//   0 1 |  0   1   1    0   1    0     1
//   1 0 |  0   1   1    0   1    0     0
//   1 1 |  1   1   0    0   0    1     0
//   x 0 |  0   x   1    x   x    x     x
//   z 1 |  x   1   x    0   x    x     x   (z acts as x)
#[test]
fn sim_gates_two_input_truth_tables_with_xz() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg a, b;
    wire y_and, y_or, y_nand, y_nor, y_xor, y_xnor, y_not;
    and  ga(y_and, a, b);
    or   go(y_or, a, b);
    nand gn(y_nand, a, b);
    nor  go2(y_nor, a, b);
    xor  gx(y_xor, a, b);
    xnor gx2(y_xnor, a, b);
    not  gt(y_not, a);
    initial begin
        a = 0; b = 0;
        #1 $display("%b_%b_%b_%b_%b_%b_%b", y_and, y_or, y_nand, y_nor, y_xor, y_xnor, y_not);
        a = 0; b = 1;
        #1 $display("%b_%b_%b_%b_%b_%b_%b", y_and, y_or, y_nand, y_nor, y_xor, y_xnor, y_not);
        a = 1; b = 0;
        #1 $display("%b_%b_%b_%b_%b_%b_%b", y_and, y_or, y_nand, y_nor, y_xor, y_xnor, y_not);
        a = 1; b = 1;
        #1 $display("%b_%b_%b_%b_%b_%b_%b", y_and, y_or, y_nand, y_nor, y_xor, y_xnor, y_not);
        a = 1'bx; b = 0;
        #1 $display("%b_%b_%b_%b_%b_%b_%b", y_and, y_or, y_nand, y_nor, y_xor, y_xnor, y_not);
        a = 1'bz; b = 1;
        #1 $display("%b_%b_%b_%b_%b_%b_%b", y_and, y_or, y_nand, y_nor, y_xor, y_xnor, y_not);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "truth").expect("simulation should run");
    assert_eq!(
        stdout,
        "0_0_1_1_0_1_1\n\
         0_1_1_0_1_0_1\n\
         0_1_1_0_1_0_0\n\
         1_1_0_0_0_1_0\n\
         0_x_1_x_x_x_x\n\
         x_1_x_0_x_x_x\n"
    );
}

// ── Multi-input gates (3 inputs, left-to-right reduce) ──────────────────────

// and(1,1,0)=0; and(1,1,1)=1.
// xor left-to-right: (1^0)^1 = 0; xnor = ~(1^0^1) = 1.
// nand(1,0,1) = ~(1&0&1) = 1; nor(1,0,1) = ~(1|0|1) = 0.
#[test]
fn sim_gates_multi_input_three_terminals() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg i0, i1, i2;
    wire y_and, y_or, y_nand, y_nor, y_xor, y_xnor;
    and  ga(y_and, i0, i1, i2);
    or   go(y_or, i0, i1, i2);
    nand gn(y_nand, i0, i1, i2);
    nor  go2(y_nor, i0, i1, i2);
    xor  gx(y_xor, i0, i1, i2);
    xnor gx2(y_xnor, i0, i1, i2);
    initial begin
        i0 = 1; i1 = 1; i2 = 0;
        #1 $display("%b_%b_%b_%b_%b_%b", y_and, y_or, y_nand, y_nor, y_xor, y_xnor);
        i0 = 1; i1 = 1; i2 = 1;
        #1 $display("%b_%b_%b_%b_%b_%b", y_and, y_or, y_nand, y_nor, y_xor, y_xnor);
        i0 = 1; i1 = 0; i2 = 1;
        #1 $display("%b_%b_%b_%b_%b_%b", y_and, y_or, y_nand, y_nor, y_xor, y_xnor);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "multi3").expect("simulation should run");
    // (1,1,0): and=0 or=1 nand=~0=1 nor=~1=0 xor=(1^1)^0=0 xnor=~0=1
    // (1,1,1): and=1 or=1 nand=0 nor=0 xor=(1^1)^1=1 xnor=0
    // (1,0,1): and=0 or=1 nand=1 nor=0 xor=(1^0)^1=0 xnor=1
    assert_eq!(stdout, "0_1_1_0_0_1\n1_1_0_0_1_0\n0_1_1_0_0_1\n");
}

// ── Vector (width-4) gates are bitwise ──────────────────────────────────────

#[test]
fn sim_gates_vector_bitwise_width4() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [3:0] va, vb;
    wire [3:0] vy_and, vy_or, vy_xor;
    and ga(vy_and, va, vb);
    or  go(vy_or, va, vb);
    xor gx(vy_xor, va, vb);
    initial begin
        va = 4'b0101; vb = 4'b0011;
        #1 $display("%b %b %b", vy_and, vy_or, vy_xor);
        va = 4'b1100; vb = 4'b1010;
        #1 $display("%b %b %b", vy_and, vy_or, vy_xor);
        va = 4'b1000; vb = 4'b00x0;
        #1 $display("%b %b %b", vy_and, vy_or, vy_xor);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "vector4").expect("simulation should run");
    // 0101&0011=0001; |=0111; ^=0110
    // 1100&1010=1000; |=1110; ^=0110
    // 1000&00x0=0000 (0 dominates); |=10x0; ^=10x0
    assert_eq!(stdout, "0001 0111 0110\n1000 1110 0110\n0000 10x0 10x0\n");
}

// ── buf / bufif0 / bufif1 / notif0 / notif1 enable semantics ────────────────

// LRM 1364-1995 §7.4 Table 7-5: an ENABLED gate acts like `buf`/`not`
// (Tables 7-3/7-4), so a data Z reaches the output as X; a DISABLED gate
// drives Z; an unknown enable yields X unless both possible outputs agree
// (the runtime mux resolves that case to the shared value).
//
//   en data | bufif1 bufif0 notif1 notif0
//   1  0    |  0      z      1      z
//   1  1    |  1      z      0      z
//   0  0    |  z      0      z      1
//   0  1    |  z      1      z      0
//   x  1    |  x      x      x      x    (driven vs Z disagree under en=x)
//   0  z    |  z      x      z      x    (bufif1/notif1 disabled -> z;
//                                           bufif0/notif0 enabled: data z
//                                           becomes x, ~x = x)
//   1  z    |  x      z      x      z    (mirror: bufif1/notif1 enabled,
//                                           bufif0/notif0 disabled)
//
// Lowering normalizes the passing arm with `data|data` (per-bit z→x per
// llg_rt.c sv4_bitwise OR) before the mux/invert so an enabled gate cannot
// leak a data Z through the copy-context mux.
#[test]
fn sim_gates_enable_gates_table_7_5() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg en, data;
    wire y_b1, y_b0, y_n1, y_n0;
    bufif1 gb1(y_b1, data, en);
    bufif0 gb0(y_b0, data, en);
    notif1 gn1(y_n1, data, en);
    notif0 gn0(y_n0, data, en);
    initial begin
        en = 1; data = 0;
        #1 $display("%b_%b_%b_%b", y_b1, y_b0, y_n1, y_n0);
        en = 1; data = 1;
        #1 $display("%b_%b_%b_%b", y_b1, y_b0, y_n1, y_n0);
        en = 0; data = 0;
        #1 $display("%b_%b_%b_%b", y_b1, y_b0, y_n1, y_n0);
        en = 0; data = 1;
        #1 $display("%b_%b_%b_%b", y_b1, y_b0, y_n1, y_n0);
        en = 1'bx; data = 1;
        #1 $display("%b_%b_%b_%b", y_b1, y_b0, y_n1, y_n0);
        en = 0; data = 1'bz;
        #1 $display("%b_%b_%b_%b", y_b1, y_b0, y_n1, y_n0);
        en = 1; data = 1'bz;
        #1 $display("%b_%b_%b_%b", y_b1, y_b0, y_n1, y_n0);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "enable").expect("simulation should run");
    assert_eq!(
        stdout,
        "0_z_1_z\n\
         1_z_0_z\n\
         z_0_z_1\n\
         z_1_z_0\n\
         x_x_x_x\n\
         z_x_z_x\n\
         x_z_x_z\n"
    );
}

// ── pullup / pulldown drive undriven wires ──────────────────────────────────

#[test]
fn sim_gates_pullup_pulldown_drive_undriven_wire() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    wire w_up, w_down, w_plain;
    pullup pu(w_up);
    pulldown pd(w_down);
    initial begin
        #1 $display("%b_%b_%b", w_up, w_down, w_plain);
        #1 $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "pull").expect("simulation should run");
    assert_eq!(stdout, "1_0_x\n");
}

// ── Gate chains settle across delta cycles ──────────────────────────────────

// not(gate out) chain: a=1 → t=~a=0 → y=~t=1 → l=~y=0.
#[test]
fn sim_gates_gate_chain_settles() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg a, b;
    wire t, y;
    reg l;
    and ga(t, a, b);
    not gb(y, t);
    always @* l = y;
    initial begin
        a = 1; b = 1;
        #1 $display("t=%b y=%b l=%b", t, y, l);
        b = 0;
        #1 $display("t=%b y=%b l=%b", t, y, l);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "chain").expect("simulation should run");
    assert_eq!(stdout, "t=1 y=0 l=0\nt=0 y=1 l=1\n");
}

// ── Gate output wakes an always_comb reader ─────────────────────────────────

#[test]
fn sim_gates_comb_process_wakes_on_gate_write() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg a;
    wire y;
    logic [2:0] hist;
    int idx = 0;
    not g(y, a);
    always_comb begin
        hist[idx] = y;
        idx = idx + 1;
    end
    initial begin
        hist = 3'b000;
        a = 0;              // gate writes y=1 -> comb records hist[1]=1
        #1 ;
        a = 1;              // gate writes y=0 -> comb records hist[2]=0
        #1 $display("%b %0d", hist, idx);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "combwake").expect("simulation should run");
    // Spawn order pins determinism: the gate comb process (Comb pass) runs
    // and settles before the always_comb (Procs pass) spawns, so the
    // always_comb's spawn-time evaluation records hist[0] = x (y = ~a with
    // a = x); the initial's own `hist = 3'b000` then overwrites that slot
    // with 0.  Each later gate write re-triggers the always_comb (it also
    // wakes on its `idx` reads): hist[1] = 1 after `a = 0`, hist[2] = 0
    // after `a = 1`.
    assert_eq!(stdout, "010 3\n");
}

// ── Gate delay lags the write behind input changes (%t timestamps) ──────────

// Spawn order pins determinism: the delayed gate registers its t=2 timer at
// t=0 (during the Comb pass) before the initial block registers anything,
// so on ties the gate always resumes first and its stale-value window is
// observable exactly as below.
#[test]
fn sim_gates_delay_lags_timestamps() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"`timescale 1ns/1ps
module tb;
    reg a;
    wire y;
    not #2 g(y, a);
    initial begin
        a = 1'b0;                          // t=0: gate will write y=1 at t=2
        #1 $display("%0t %b", $time, y);   // t=1: first write pending -> x
        #1 $display("%0t %b", $time, y);   // t=2: y=1 landed
        a = 1'b1;                          // t=2: gate will write y=0 at t=4
        #1 $display("%0t %b", $time, y);   // t=3: still 1
        #1 $display("%0t %b", $time, y);   // t=4: y=0 landed
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "delay").expect("simulation should run");
    assert_eq!(stdout, "1 x\n2 1\n3 1\n4 0\n");
}

// ── Parameterized gate delay folds through the parameter value ─────────────

#[test]
fn sim_gates_parameterized_delay() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"`timescale 1ns/1ps
module tb #(parameter D = 3) ();
    reg a;
    wire y;
    and #D g(y, a, a2);
    wire a2 = 1'b1;
    initial begin
        a = 1'b1;                          // t=0: gate writes y=1 at t=D=3
        #2 $display("%0t %b", $time, y);   // t=2: pending -> x
        #2 $display("%0t %b", $time, y);   // t=4: written
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "pardelay").expect("simulation should run");
    assert_eq!(stdout, "2 x\n4 1\n");
}

/// A delayed gate rejects a short pulse and schedules the later stable
/// transition using inertial delay semantics.
#[test]
#[ignore = "DELAY-BUG: delayed gates need inertial update scheduling"]
fn sim_gates_delay_rejects_short_pulse() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg a;
    wire y;
    not #3 g(y, a);

    initial begin
        a = 1'b0;
        #1 a = 1'b1;
        #1 a = 1'b0;
        #1 $display("t=%0t y=%b", $time, y);
        #1 a = 1'b1;
        #2 $display("t=%0t y=%b", $time, y);
        #1 $display("t=%0t y=%b", $time, y);
        $finish;
    end
endmodule
"#;

    // IEEE inertial scheduling rejects the short pulse, leaving y unknown
    // until the stable transition scheduled for t=7.
    let stdout = run_sim(sv, "current").expect("simulation should run");
    assert_eq!(stdout, "t=3 y=x\nt=6 y=x\nt=7 y=0\n");
}

// ── Gates inside generate scopes elaborate per iteration ────────────────────

#[test]
fn sim_gates_inside_generate_scope() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    // Each iteration gets its own per-iteration gate and initial block
    // under `g_0_`/`g_1_`; the genvar comparison folds to a per-iteration
    // constant.
    let sv = r#"module tb;
    genvar i;
    for (i = 0; i < 2; i = i + 1) begin : g
        reg ai;
        wire yi;
        not gi(yi, ai);
        initial begin
            ai = (i == 0);
            #1 $display("g%0d %b_%b", i, ai, yi);
        end
    end
    initial begin
        #3 $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "genscope").expect("simulation should run");
    // Iteration 0: ai = (i == 0) = 1 -> yi = 0; iteration 1 mirrors it.
    assert_eq!(stdout, "g0 1_0\ng1 0_1\n");
}

// ── Rejections ───────────────────────────────────────────────────────────────

#[test]
fn sim_gates_reject_udp_instance() {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"primitive mux2 (out, sel, a, b);
    output out;
    input sel, a, b;
    table
        0 ? 1 : 0 ;
        0 0 ? : 0 ;
        1 ? 0 : 1 ;
        1 1 ? : 1 ;
        x 0 0 : 0 ;
        x 1 1 : 1 ;
    endtable
endprimitive

module tb;
    reg sel, a, b;
    wire y;
    mux2 u(y, sel, a, b);
    initial begin
        sel = 0; a = 0; b = 1;
        $finish;
    end
endmodule
"#;
    let err = codegen_error(sv, "udp").expect("compile should succeed");
    assert!(
        err.contains("user-defined primitive"),
        "unexpected error: {err}"
    );
}

#[test]
fn sim_gates_reject_switch_primitive() {
    let _guard = SURELOG_LOCK.lock().unwrap();
    // Surelog's UHDM lint rejects wire terminals on switch primitives
    // ("Illegal lhs of type wire"), so this design uses regs to reach the
    // codegen boundary being pinned here.
    let sv = r#"module tb;
    reg y, a;
    tran t1(y, a);
    initial begin
        a = 1;
        $finish;
    end
endmodule
"#;
    let err = codegen_error(sv, "switch").expect("compile should succeed");
    assert!(
        err.contains("switch/transistor primitive"),
        "unexpected error: {err}"
    );
}

#[test]
fn sim_gates_reject_gate_array() {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg a, b;
    wire [3:0] y;
    and g[3:0] (y, a, b);
    initial begin
        a = 1; b = 1;
        $finish;
    end
endmodule
"#;
    let err = codegen_error(sv, "array").expect("compile should succeed");
    assert!(
        err.contains("primitive array") && err.contains("not supported"),
        "unexpected error: {err}"
    );
}

// ── Gate output drives a collapsed inout-net member ─────────────────────────

// A gate whose output terminal is a net collapsed into an inout-port group
// writes through the group's driver slot (same path as a continuous-
// assignment LHS), so the parent sees the resolved value.
#[test]
fn sim_gates_gate_output_drives_inout_net_member() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"`timescale 1ns/1ps
module child(input wire a, b, inout wire y);
    and g(y, a, b);
endmodule

module tb;
    reg a, b;
    wire y;
    child u(.a(a), .b(b), .y(y));
    initial begin
        a = 1; b = 1;
        #1 $display("%b", y);
        b = 0;
        #1 $display("%b", y);
        $finish;
    end
endmodule
"#;
    let stdout = run_sim(sv, "gatenet").expect("simulation should run");
    assert_eq!(stdout, "1\n0\n");
}

#[test]
fn sim_gates_reject_select_terminal() {
    let _guard = SURELOG_LOCK.lock().unwrap();
    // Select-connected terminals are a clean v1 reject: gates drive/read
    // whole signals only.
    let sv = r#"module tb;
    reg [3:0] wide;
    wire y;
    and g(y, wide[1], wide[3]);
    initial begin
        wide = 4'b1010;
        $finish;
    end
endmodule
"#;
    let err = codegen_error(sv, "bitsel").expect("compile should succeed");
    assert!(
        err.contains("whole plain signals"),
        "unexpected error: {err}"
    );
}

#[test]
fn sim_gates_reject_mixed_width_terminals() {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg a;
    reg [3:0] wide;
    wire y;
    and g(y, a, wide);
    initial begin
        a = 1;
        $finish;
    end
endmodule
"#;
    let err = codegen_error(sv, "widthmix").expect("compile should succeed");
    assert!(
        err.contains("different widths") && err.contains("equal terminal widths"),
        "unexpected error: {err}"
    );
}
