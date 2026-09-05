//! End-to-end simulator tests for `final begin … end` blocks (SV 1800-2005
//! §10.7): Surelog compile → codegen → CMake build → run, asserting exact
//! stdout against hand-simulated traces.
//!
//! Regression coverage: finals execute ONCE after the scheduler exits
//! ($finish ordering), they observe NBA-committed values and the end-of-run
//! time, a deadlock termination still runs them, multiple finals run in
//! source order, `$finish` inside a final terminates the final phase, timing
//! controls inside a final are clean codegen
//! rejects, nonblocking assignments, task calls, and deferred output tasks are
//! rejected,
//! and optimizer on/off runs agree.
//!
//! Note: Surelog parses `final` only in `.sv` files (frontend limitation),
//! which every design here satisfies.

#[path = "support/sim.rs"]
mod sim_harness;

use std::sync::Mutex;

use llg::core::compile;
use llg::sim;
use llg::sim::opt::OptConfig;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Compile, generate, build, and run one design.
fn run_sim(sv: &str, tag: &str) -> Result<(String, String), String> {
    let run = sim_harness::run_generated_sim(sv, "tb", tag)?;
    Ok((run.stdout, run.stderr))
}

/// Compile + codegen `sv`, returning the codegen error message.
/// Holds [`SURELOG_LOCK`] for the whole pipeline.
fn codegen_error(sv: &str, tag: &str) -> Result<String, String> {
    let _guard = SURELOG_LOCK.lock().unwrap();
    sim_harness::with_temp_cwd(tag, |dir| {
        let src = dir.join("tb.sv");
        std::fs::write(&src, sv).map_err(|error| format!("write source: {error}"))?;
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
            Ok(_) => panic!("codegen should reject the design ({tag})"),
            Err(e) => Ok(e.to_string()),
        }
    })
}

/// (a) A final block runs exactly once AFTER the scheduler exits: its output
/// lands after the last procedural display, and it observes the final signal
/// values.
#[test]
fn sim_final_runs_after_finish() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] x;

    initial begin
        x = 8'h5a;
        $display("mid x=%h", x);
        #10 x = 8'h11;
        $display("end x=%h", x);
        $finish;
    end

    final begin
        $display("final sees x=%h", x);
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  x=5a; display "mid x=5a".
    //   t=10 x=11; display "end x=11"; $finish stops the scheduler.
    //   THEN the finals phase runs: "final sees x=11".
    //
    // Expected stdout (exactly):
    //   mid x=5a
    //   end x=11
    //   final sees x=11
    let (stdout, _stderr) = run_sim(sv, "afterfinish").expect("simulation should run");
    assert_eq!(stdout, "mid x=5a\nend x=11\nfinal sees x=11\n");
}

/// (b) Finals observe NBA-committed values and the END-OF-RUN time: four
/// posedges increment `cnt` through the NBA region before $finish, and
/// `$time` inside the final reports the time of the last scheduler event.
#[test]
fn sim_final_sees_nba_values_and_end_time() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk = 1'b0;
    reg [3:0] cnt = 4'd0;

    always #5 clk = ~clk;
    always @(posedge clk) cnt <= cnt + 4'd1;

    initial #22 $finish;

    final begin
        $display("cnt=%0d at t=%0t", cnt, $time);
    end
endmodule
"#;

    // Hand-simulation: clk toggles at t=5/10/15/20, so the POSEDGES are at
    // t=5 and t=15 only; each commits one NBA increment -> cnt=2.  $finish
    // fires at t=22.  The final reads the committed cnt=2 and the end-of-run
    // time 22.
    //
    // Expected stdout (exactly):
    //   cnt=2 at t=22
    let (stdout, _stderr) = run_sim(sv, "nba").expect("simulation should run");
    assert_eq!(stdout, "cnt=2 at t=22\n");
}

/// Final procedures permit function statements only, so a nonblocking
/// assignment is rejected instead of creating an NBA after simulation ends.
#[test]
fn sim_final_rejects_nonblocking_assignment() {
    let sv = r#"module tb;
    reg [7:0] x;

    initial begin
        x = 8'h11;
        $finish;
    end

    final begin
        x <= 8'h22;
    end
endmodule
"#;

    let err = codegen_error(sv, "final-nba").expect("compile should succeed");
    assert!(
        err.contains("nonblocking assignment inside a final block"),
        "unexpected codegen error: {err}"
    );
}

/// (c) A DEADLOCK termination (waiters left, no future events) also runs the
/// finals phase — the runtime exits the scheduler loop and finals still
/// execute.
#[test]
fn sim_final_runs_after_deadlock() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    event ev;

    initial begin
        @(ev);
        $display("never printed");
    end

    final begin
        $display("final ran after deadlock");
    end
endmodule
"#;

    // Hand-simulation: nobody ever triggers `ev`, no timed events remain ->
    // the scheduler reports a deadlock on STDERR and returns; the finals
    // phase then prints the final line on stdout.
    //
    // Expected stdout (exactly):
    //   final ran after deadlock
    let (stdout, stderr) = run_sim(sv, "deadlock").expect("simulation should run");
    assert_eq!(stdout, "final ran after deadlock\n");
    assert!(
        stderr.contains("simulation deadlock"),
        "expected the deadlock report on stderr, got: {stderr}"
    );
}

/// (d) `$finish` inside a final terminates simulation immediately: statements
/// after it and all remaining finals are skipped.
#[test]
fn sim_final_finish_terminates_final_phase() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    initial begin
        #5 $display("body t=5");
        $finish;
    end

    final begin
        $display("final one");
        $finish;
        $display("final one continues");
    end

    final begin
        $display("final two");
    end
endmodule
"#;

    // Expected stdout (exactly):
    //   body t=5
    //   final one
    // Neither the remainder of final one nor final two executes.
    let (stdout, stderr) = run_sim(sv, "finishwarn").expect("simulation should run");
    assert_eq!(stdout, "body t=5\nfinal one\n");
    assert!(
        !stderr.contains("$finish inside a final block"),
        "unexpected obsolete warning: {stderr}"
    );
}

/// (e) Timing controls inside a final are clean codegen rejects (LRM
/// 1800-2005 §10.7).
#[test]
fn sim_final_rejects_timing_controls() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let cases: [(&str, &str); 3] = [
        (
            r#"module tb;
    final begin
        #5 $display("delayed");
    end
endmodule
"#,
            "#delay",
        ),
        (
            r#"module tb;
    reg a;
    final begin
        @(posedge a) $display("edged");
    end
endmodule
"#,
            "@(...)",
        ),
        (
            r#"module tb;
    reg ready;
    final begin
        wait (ready) $display("waited");
    end
endmodule
"#,
            "wait (...)",
        ),
    ];
    for (i, (sv, needle)) in cases.iter().enumerate() {
        let err = codegen_error(sv, &format!("reject{i}")).expect("compile should succeed");
        assert!(
            err.contains("no timing controls in final"),
            "case {needle}: unexpected codegen error: {err}"
        );
    }
}

/// (f) Optimizer parity: an opt-on run and an opt-off run of a final-block
/// design must produce byte-identical stdout.
#[test]
fn sim_final_opt_parity() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = sim_harness::TempDir::new("final-parity").expect("create temp dir");
    let src = dir.path().join("tb.sv");
    let sv = r#"module tb;
    reg [3:0] acc;
    integer i;

    initial begin
        acc = 4'd0;
        for (i = 0; i < 6; i = i + 1) acc = acc + i[3:0];
        $display("acc=%0d", acc);
        $finish;
    end

    final begin
        $display("final acc=%0d double=%0d", acc, acc * 2);
    end
endmodule
"#;
    std::fs::write(&src, sv).expect("write source");

    let _guard = SURELOG_LOCK.lock().unwrap();
    let result = sim_harness::with_cwd(dir.path(), || {
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
        let db = llg::core::db::Db::build(design).map_err(|e| format!("db: {e}"))?;
        let on = sim::codegen::generate_from_db_with_opts(&db, &OptConfig::default())
            .map_err(|e| format!("codegen(opt-on): {e}"))?;
        let off = sim::codegen::generate_from_db_with_opts(&db, &OptConfig::none())
            .map_err(|e| format!("codegen(opt-off): {e}"))?;

        let run = |name: &str, model_c: &str| -> Result<String, String> {
            let out_dir = dir.path().join(name);
            let exe = sim::build::build_model_cmake(&out_dir, &[("model.c", model_c)])
                .map_err(|e| format!("cmake({name}): {e}"))?;
            sim_harness::run_executable(&exe).map_err(|error| format!("run({name}): {error}"))
        };
        // acc = 0+1+2+3+4+5 = 15 (fits 4 bits exactly).
        let expected = "acc=15\nfinal acc=15 double=30\n";
        let on_out = run("opt_on", &on.model_c)?;
        assert_eq!(on_out, expected, "opt-on run diverged");
        assert_eq!(run("opt_off", &off.model_c)?, on_out, "parity broken");
        Ok(())
    });
    result.expect("parity simulation should run");
}

/// (g) Deferred output tasks are rejected in a final because no scheduled
/// events execute after final procedures finish.
#[test]
fn sim_final_rejects_deferred_output_tasks() {
    for (i, task) in ["$strobe", "$monitor"].iter().enumerate() {
        let sv =
            format!("module tb; reg x; initial $finish; final {task}(\"x=%b\", x); endmodule\n");
        let err = codegen_error(&sv, &format!("deferred-{i}")).expect("compile should succeed");
        assert!(
            err.contains("no scheduled output events execute after final procedures"),
            "{task}: unexpected codegen error: {err}"
        );
    }
}

/// Task calls are not function-legal statements and may suspend or otherwise
/// schedule work after the final phase has begun.
#[test]
fn sim_final_rejects_task_calls() {
    let sv = r#"module tb;
    task report;
        $display("task ran");
    endtask

    initial $finish;
    final report();
endmodule
"#;
    let err = codegen_error(sv, "task-call").expect("compile should succeed");
    assert!(
        err.contains("task call `report` inside a final block"),
        "unexpected codegen error: {err}"
    );
}
