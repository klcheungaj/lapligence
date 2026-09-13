//! End-to-end simulator tests for procedural `force` / `release` and
//! procedural continuous assignment (`assign <reg> = expr;` /
//! `deassign <reg>;`, LRM 1364-1995 §9.4 / 2001 §9.5): Slang compile →
//! codegen → CMake build → run, asserting exact stdout against hand-simulated
//! traces.
//!
//! Regression coverage: force overrides a process (blocking) write; force
//! overrides a continuous assign; force wakes `@(sig)` waiters; a
//! non-blocking write to a forced target is dropped at NBA commit;
//! assign/deassign lifecycle (value holds after deassign), RHS propagation
//! while assigned, re-assign after deassign re-enables, force overriding an
//! active PCA with release resuming it, clean codegen rejects (net
//! target and select target), a `deassign` whose process
//! LOWERS before the process carrying the matching `assign` (two-phase site
//! discovery), and a loop revisiting an earlier `deassign`.
//!
//! Tests run with the CWD pointed at a fresh temp dir and serialize process-CWD
//! changes with the other native integration tests.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

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
    //        prints "consumer at 2 w=1" and re-registers on w.
    //   t=4  release w retains 1, so no second event is generated.
    //   t=6  $finish.
    //
    // Expected stdout (exactly):
    //   consumer at 2 w=1
    let (stdout, _warnings, _model) = run_sim(sv, "tb", "wake").expect("simulation should run");
    assert_eq!(stdout, "consumer at 2 w=1\n");
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

/// (e) Procedural continuous assignment lifecycle (LRM 1364-1995 §9.4):
/// `assign x = src;` drives x immediately and continuously; while assigned,
/// RHS changes propagate; `deassign x;` leaves x HOLDING its last value; a
/// later execution of the same `assign` statement re-enables the site and
/// picks up the current RHS.
#[test]
fn sim_pca_lifecycle() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [7:0] x;
    reg [7:0] src;
    reg en;

    // One PCA site, re-executed by control flow (a second textual
    // `assign x = …` on the same variable is a static codegen reject).
    always @(en or src) begin
        if (en) assign x = src;
        else deassign x;
    end

    initial begin
        src = 8'h11;
        en = 1'b1;
        #2 src = 8'h22;
        #2 $display("x=%h t=%0t", x, $time);
        en = 1'b0;
        src = 8'h33;
        #2 $display("x=%h t=%0t", x, $time);
        en = 1'b1;
        #2 $display("x=%h t=%0t", x, $time);
        $finish;
    end
endmodule
"#;

    // Hand-simulation (spawn order: always, PCA guard for x, initial):
    //   t=0  initial: src=11, en=1.  The always wakes and executes
    //        `assign x = src`: enable=1 plus an immediate write x=11.  The
    //        guard wakes on the same changes and writes the current rhs.
    //   t=2  src=22 -> guard + immediate statement write drive x=22.
    //   t=4  display -> "x=22 t=4".  en=0 executes `deassign x` (enable=0;
    //        x KEEPS its value).  src=33: the guard wakes but is disabled.
    //   t=6  display -> "x=22 t=6" (held).  en=1 re-executes
    //        `assign x = src` with the CURRENT src: x=33 immediately; the
    //        guard agrees.
    //   t=8  display -> "x=33 t=8".
    //
    // Expected stdout (exactly):
    //   x=22 t=4
    //   x=22 t=6
    //   x=33 t=8
    let (stdout, _warnings, _model) = run_sim(sv, "tb", "pcalife").expect("simulation should run");
    assert_eq!(stdout, "x=22 t=4\nx=22 t=6\nx=33 t=8\n");
}

/// (f) force/release priority over an active procedural continuous
/// assignment (LRM 1800-2005 §10.6.2): while x is forced, the PCA's writes
/// are dropped like any other procedural write; `release` resumes the latest
/// live PCA value, and the next RHS change shows the continuous assignment
/// driving again.
#[test]
fn sim_pca_force_overrides_and_release_restores() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [7:0] x;
    reg [7:0] src;

    initial begin
        src = 8'h01;
        assign x = src;
        #2 force x = 8'hff;
        #2 src = 8'h02;
        #2 $display("x=%h", x);
        release x;
        #2 src = 8'h04;
        #2 $display("x=%h", x);
        $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  src=01; `assign x = src` writes x=01 and enables the site.
    //   t=2  force x=ff (the live PCA remains underneath).
    //   t=4  src=02: the PCA guard wakes and issues llg_ba(x, 02), which
    //        is DROPPED because x is forced.  x stays ff.
    //   t=6  display -> "x=ff".  release x resumes the latest PCA value 02.
    //   t=8  src=04: the PCA drives again -> x=04.
    //   t=10 display -> "x=04".  $finish.
    //
    // Expected stdout (exactly):
    //   x=ff
    //   x=04
    let (stdout, _warnings, _model) = run_sim(sv, "tb", "pcaforce").expect("simulation should run");
    assert_eq!(stdout, "x=ff\nx=04\n");
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

/// (i) Two textual PCA sites targeting one variable replace each other when
/// they execute, and deassign leaves the replacement site's last value.
#[test]
fn sim_pca_multi_site_replaced() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [7:0] x;
    initial begin
        assign x = 8'h01;
        #1 $display("first=%h", x);
        assign x = 8'h02;
        #1 $display("second=%h", x);
        deassign x;
        x = 8'h03;
        $display("deassigned=%h", x);
        $finish;
    end
endmodule
"#;
    let (stdout, _warnings, _model) = run_sim(sv, "tb", "pcamulti").expect("simulation should run");
    assert_eq!(stdout, "first=01\nsecond=02\ndeassigned=03\n");
}

/// (j) Order independence of site discovery: the `deassign` process LOWERS
/// before the process carrying the matching `assign` (db child order =
/// source order).  Sites are pre-allocated by a pre-scan over every process
/// body, so the deassign still clears the right enable: while assigned q is
/// driven; after the rst pulse (deassign) q HOLDS its last value instead of
/// following d forever.
#[test]
fn sim_pca_deassign_before_assign_across_processes() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [7:0] q;
    reg [7:0] d;
    reg rst;

    // Lowers FIRST — its site only exists because discovery pre-scans
    // every process body before any body lowers.
    always @(rst) begin
        if (rst) deassign q;
    end

    always @(d or rst) begin
        if (!rst) assign q = d;
    end

    initial begin
        d = 8'ha5;
        rst = 1'b1;
        #2 rst = 1'b0;
        #2 d = 8'hb2;
        #2 $display("q=%h t=%0t", q, $time);
        rst = 1'b1;
        #2 $display("q=%h t=%0t", q, $time);
        d = 8'hc3;
        #2 $display("q=%h t=%0t", q, $time);
        $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  initial: d=a5, rst=1.  The deassign process wakes and executes
    //        `deassign q` (enable := 0; already disabled).  The assign
    //        process sees rst=1 and does nothing.
    //   t=2  rst=0: the assign executes `assign q = d`: enable=1 plus an
    //        immediate write q=a5; the guard agrees.
    //   t=4  d=b2 -> q=b2 (statement write + guard).
    //   t=6  display -> "q=b2 t=6".  rst=1 executes `deassign q`
    //        (enable := 0; q KEEPS b2).  The guard is disabled now.
    //   t=8  display -> "q=b2 t=8" (held).
    //   t=8+ d=c3: the disabled guard must NOT re-drive q.
    //   t=10 display -> "q=b2 t=10".
    //
    // Without two-phase discovery the deassign lowered to a permanent no-op:
    // the guard would still be enabled and print "q=c3" at t=10.
    //
    // Expected stdout (exactly):
    //   q=b2 t=6
    //   q=b2 t=8
    //   q=b2 t=10
    let (stdout, warnings, _model) =
        run_sim(sv, "tb", "pcadeorder").expect("simulation should run");
    assert_eq!(stdout, "q=b2 t=6\nq=b2 t=8\nq=b2 t=10\n");
    assert!(
        !warnings.iter().any(|w| w.contains("has no effect")),
        "the deassign must resolve its pre-scanned site, got warnings: {warnings:?}"
    );
}

/// (k) A loop revisiting an earlier `deassign` still resolves the same site
/// on every iteration: the deassign is the SOLE clearer, textually precedes
/// the matching `assign`, and executes again on each pass — including while
/// the site is still enabled from the previous iteration.  After each
/// revisit the variable HOLDS while the RHS keeps changing.
#[test]
fn sim_pca_loop_revisits_earlier_deassign() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [7:0] x;
    reg [7:0] src;
    integer i;

    initial begin
        src = 8'h05;
        for (i = 0; i < 2; i = i + 1) begin
            deassign x;
            #2 src = src + 8'h01;
            #2 $display("mid x=%h", x);
            #2 assign x = src;
            #2 src = src + 8'h10;
            #2 $display("driven x=%h", x);
        end
        $finish;
    end
endmodule
"#;

    // Hand-simulation (one PCA site on x, claimed by the pre-scan):
    //   iter 1: t=0 deassign (disabled).  t=2 src=06: nothing drives x.
    //        t=4 "mid x=xx".  t=6 assign enables + writes x=06.  t=8
    //        src=16 -> the guard drives x=16.  t=10 "driven x=16".
    //   iter 2: t=10 deassign AGAIN (enable := 0; x KEEPS 16).  t=12
    //        src=17 but the guard is disabled.  t=14 "mid x=16" — the
    //        held value.  t=16 assign re-enables with the current src:
    //        x=17.  t=18 src=27 -> guard drives.  t=20 "driven x=27".
    //
    // Without two-phase discovery the deassign lowered to a permanent no-op
    // and the still-enabled guard would print "mid x=17" in iteration 2.
    //
    // Expected stdout (exactly):
    //   mid x=xx
    //   driven x=16
    //   mid x=16
    //   driven x=27
    let (stdout, warnings, _model) = run_sim(sv, "tb", "pcaloop").expect("simulation should run");
    assert_eq!(stdout, "mid x=xx\ndriven x=16\nmid x=16\ndriven x=27\n");
    assert!(
        !warnings.iter().any(|w| w.contains("has no effect")),
        "the revisited deassign must resolve its pre-scanned site, got warnings: {warnings:?}"
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
