//! End-to-end simulator tests for procedural `force` / `release`: Slang
//! compile → codegen → CMake build → run, asserting exact stdout against
//! hand-simulated traces.
//!
//! Regression coverage: force overrides a process (blocking) write; force
//! overrides a continuous assign; force wakes `@(sig)` waiters; a
//! non-blocking write to a forced target is dropped at NBA commit. Procedural
//! continuous assignment (`assign`/`deassign`) is unsupported by design: its
//! frontend rejects (net and select targets) and the generation-time ADV-032
//! rejection of legal forms are pinned here.
//!
//! Tests run with the CWD pointed at a fresh temp dir and serialize process-CWD
//! changes with the other native integration tests.

use crate::sim_cli;
use crate::sim_harness;

use std::sync::Mutex;

use llg::ffi::slang::DiagnosticSeverity;

static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Compile, generate, build, and run one design.
fn run_sim(sv: &str, top: &str, tag: &str) -> Result<(String, Vec<String>, String), String> {
    let run = sim_harness::run_generated_sim(sv, top, tag)?;
    Ok((run.stdout, run.warnings, run.model_c))
}

/// (a) Force overrides a process write: a blocking write to a forced reg is
/// ignored, and `release` leaves the variable at its forced value until a
/// later ordinary write.
#[test]
fn sim_force_overrides_process_write() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [7:0] x;

    initial begin
        x = 8'h01;
        force x = 8'hff;
        #2 x = 8'h02;
        $display("x=%h", x);
        release x;
        $display("x=%h", x);
    end

    initial #20 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  x = 1 (blocking).  force x = ff -> x=ff.  The initial delays #2.
    //   t=2  x = 2 is a procedural write to a FORCED signal -> dropped (LRM
    //        10.6.2).  $display reads x -> "x=ff".  release x retains the
    //        forced value, so the second display is also "x=ff".
    //   t=20 $finish.
    //
    // Expected stdout (exactly):
    //   x=ff
    //   x=ff

    let (stdout, _warnings, _model) =
        run_sim(sv, "tb", "procwrite").expect("simulation should run");
    assert_eq!(stdout, "x=ff\nx=ff\n");
}

/// (b) Force overrides a continuous assign: the wire reads the forced value
/// while forced; `release` resolves the current value the assign has produced.
#[test]
fn sim_force_overrides_continuous_assign() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg a = 1'b1;
    wire w;
    assign w = a;

    initial begin
        #1 force w = 1'b0;
        #1 $display("w=%b", w);
        release w;
        $display("w=%b", w);
        #1 $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  the continuous-assign comb process evaluates w = a = 1.
    //   t=1  force w = 0 -> w=0 while the driver's current value remains 1.
    //   t=2  $display -> "w=0".  release w resolves the current driver -> "w=1".
    //   t=3  $finish.
    //
    // Expected stdout (exactly):
    //   w=0
    //   w=1

    let (stdout, _warnings, _model) = run_sim(sv, "tb", "netforce").expect("simulation should run");
    assert_eq!(stdout, "w=0\nw=1\n");
}

/// (c) Force wakes waiters: writing the forced value through the normal write
/// path fires an `@(w)` event; releasing a variable retains the forced value
/// and therefore does not create a second change.
#[test]
fn sim_force_wakes_waiters() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg w = 0;

    always @(w) begin
        $display("consumer at %0t w=%b", $time, w);
    end

    initial begin
        #2 force w = 1'b1;
        #2 release w;
        #2 $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  the always process registers an @(w) wait (w=0); the initial
    //        delays #2.
    //   t=2  force w = 1: 0->1, the change wakes the always process, which
    //        prints "consumer at 2000 w=1" in the default design-precision
    //        units and re-registers on w.
    //   t=4  release w retains 1, so no second event is generated.
    //   t=6  $finish.
    //
    // Expected stdout (exactly):
    //   consumer at 2000 w=1
    let (stdout, _warnings, _model) = run_sim(sv, "tb", "wake").expect("simulation should run");
    assert_eq!(stdout, "consumer at 2000 w=1\n");
}

/// (d) An NBA to a forced target is dropped at commit: `x <= 8'h5a` on a
/// posedge must not overwrite the forced value.
#[test]
fn sim_force_nba_dropped() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg clk = 0;
    reg [7:0] x = 8'h00;

    always #1 clk = ~clk;

    always @(posedge clk) x <= 8'h5a;

    initial begin
        force x = 8'hff;
        #4 $display("x=%h", x);
        $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  force x = ff.  The clock toggler and the posedge waiter suspend;
    //        the initial delays
    //        #4.
    //   t=1  clk 0->1 wakes the posedge process, which records x <= 5a.  The
    //        NBA commit drops it because x is forced: x stays ff.
    //   t=3  second posedge: same, x stays ff.
    //   t=4  $display -> "x=ff", then $finish.
    //
    // Expected stdout (exactly):
    //   x=ff

    let (stdout, _warnings, _model) = run_sim(sv, "tb", "nbadrop").expect("simulation should run");
    assert_eq!(stdout, "x=ff\n");
}

/// (e) Procedural continuous assignment (`assign`/`deassign`, IEEE
/// 1364-2001 9.3.1) is unsupported by design in every form: generation stops
/// with one located ADV-032 diagnostic per statement before any C exists,
/// including the control-flow re-executed site and the `deassign` that
/// precedes its `assign` in another process.
#[test]
fn sim_procedural_assign_is_rejected_before_generation() {
    let sv = r#"module tb;
    reg [7:0] x, q, d;
    reg [7:0] src;
    reg en, rst;
    always @(en or src) begin
        if (en) assign x = src;
        else deassign x;
    end
    always @(posedge rst) deassign q;
    initial assign q = d;
endmodule
"#;
    let error = sim_harness::with_frontend_temp_cwd("pcareject", |dir| {
        let path = dir.join("tb.sv");
        std::fs::write(&path, sv).map_err(|error| error.to_string())?;
        let compiled = llg::core::compile::compile_checked(&llg::core::compile::CompileOpts {
            files: vec![path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| error.to_string())?;
        let db =
            llg::core::db::Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        match llg::sim::codegen::generate(&db) {
            Ok(_) => Err("procedural assign unexpectedly generated".to_owned()),
            Err(error) => Ok(error),
        }
    })
    .expect("procedural assign rejection must reach generation");
    assert!(error.is_legacy_unsupported(), "{error}");
    let lines: Vec<_> = error
        .detail()
        .lines()
        .map(|line| line.rsplit_once("tb.sv:").map_or(line, |(_, tail)| tail))
        .collect();
    let family = "(legacy procedural assign/deassign form) is not supported by llg";
    assert_eq!(
        lines,
        [
            format!("6:17: unsupported: procedural `assign` {family}"),
            format!("7:14: unsupported: procedural `deassign` {family}"),
            format!("9:27: unsupported: procedural `deassign` {family}"),
            format!("10:13: unsupported: procedural `assign` {family}"),
        ]
    );
}

/// (g) Frontend rejection: PCA on a net target (LRM: variables only).
#[test]
fn sim_pca_net_target_rejected() {
    let sv = r#"module tb;
    wire w;
    assign w = 1'b1;
    initial begin
        assign w = 1'b0;
    end
    initial #5 $finish;
endmodule
"#;
    let diagnostics = sim_harness::frontend_diagnostics(sv, "tb").expect("compile net PCA");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Error
                && diagnostic.name == "BadProceduralAssign"
        }),
        "net PCA must report BadProceduralAssign: {diagnostics:?}"
    );
}

/// (h) Frontend rejection: PCA on a part-select target (whole variables
/// only).
#[test]
fn sim_pca_select_target_rejected() {
    let sv = r#"module tb;
    reg [7:0] x;
    reg c;
    always @(c) begin
        assign x[3:0] = 4'ha;
    end
    initial #5 $finish;
endmodule
"#;
    let diagnostics = sim_harness::frontend_diagnostics(sv, "tb").expect("compile select PCA");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Error
                && diagnostic.name == "BadProceduralAssign"
        }),
        "select PCA must report BadProceduralAssign: {diagnostics:?}"
    );
}

#[test]
fn sim_force_rhs_re_evaluates_from_checked_in_fixture() {
    sim_cli::run_case(
        "force",
        "Force_RHS_Reevaluation",
        "CHECK: initial=1\nCHECK: follows=0\n",
        "",
        &[],
    );
}

#[test]
fn sim_release_variable_retains_forced_value_from_fixture() {
    sim_cli::run_case(
        "force",
        "Release_Variable_Retains_Forced_Value",
        "CHECK: retained=1\nCHECK: next_write=0\n",
        "",
        &[],
    );
}

#[test]
fn sim_release_net_resolves_current_driver_from_fixture() {
    sim_cli::run_case(
        "force",
        "Release_Net_Resolves_Current_Driver",
        "CHECK: resolved=1\n",
        "",
        &[],
    );
}

#[test]
fn sim_force_advanced_contexts_and_real_nba() {
    sim_cli::run_case(
        "force",
        "Force_Advanced_Contexts",
        "hier_initial=01\n\
hier_live=03\n\
hier_replaced=06\n\
hier_released=06\n\
hier_write=04\n\
real_nba=1.0\n\
real_live=3.0\n\
real_release=3.0\n",
        "",
        &[],
    );
}

#[test]
fn sim_force_selected_net_overlay_and_resolution() {
    sim_cli::run_case(
        "force",
        "Force_Selected_Net_Overlay",
        "forced=0010\n\
upper_release=1110\n\
all_release=1100\n\
conflict=xxxx\n\
z_release=0011\n",
        "",
        &[],
    );
}

#[test]
fn sim_force_rejects_automatic_targets() {
    sim_cli::reject_case(
        "force",
        "Force_Illegal_Automatic_Target",
        "automatic variable",
    );
}

#[test]
fn sim_force_rejects_variable_selects() {
    sim_cli::reject_case(
        "force",
        "Force_Illegal_Variable_Select",
        "non-constant variable",
    );
}
