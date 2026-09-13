//! End-to-end simulator tests for `wait (cond) stmt;` support: Slang compile
//! → codegen → CMake build → run, asserting exact stdout against hand-simulated
//! traces.
//!
//! Regression coverage: level-sensitive handshake with multiple waiters waking
//! on the same change, immediate constant-true conditions, wait-then-body data
//! writes, wait-bearing task inlining, and compound conditions waking only
//! when the whole expression becomes true.
//!
//! These tests temporarily change the process working directory, so the
//! tests run with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, to avoid process-wide CWD races).

use llg::core::compile;
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

/// Compile + codegen + C-compile + run `sv` (top module `top`), returning the
/// simulator's exact stdout, the codegen warnings and the generated C model.
fn run_sim(sv: &str, top: &str, tag: &str) -> Result<(String, Vec<String>, String), String> {
    sim_harness::with_frontend_temp_cwd(tag, |dir| {
        let source = dir.join("tb.sv");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some(top.to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let db =
            llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
        let generated = sim::codegen::generate(&db).map_err(|error| format!("codegen: {error}"))?;
        let executable =
            sim::build::build_model_cmake(dir, &[("model.c", generated.model_c.as_str())])
                .map_err(|error| format!("cmake: {error}"))?;
        let stdout = sim_harness::run_executable(&executable)?;
        Ok((stdout, generated.warnings, generated.model_c))
    })
}

/// (a) Handshake: two consumers both `wait (ready)` while a producer raises
/// `ready` at t=5.  `wait` is level-sensitive, so every waiter registered on
/// `ready` wakes on the change and prints at t=5.
#[test]
fn sim_wait_handshake_two_consumers() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg ready = 0;

    initial begin
        wait (ready);
        $display("c1 at %0t", $time);
    end

    initial begin
        wait (ready);
        $display("c2 at %0t", $time);
    end

    initial begin
        #5 ready = 1;
    end

    initial #20 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  both consumers evaluate wait(ready): ready=0 (declaration
    //        initializer) -> false; each registers a level waiter on ready.
    //        The producer delays #5.
    //   t=5  producer: ready 0->1.  The change wakes both consumers (level
    //        trigger); each re-evaluates ready -> true and prints.  The
    //        runtime wakes the waiters in reverse registration order, so the
    //        second consumer (registered after the first) prints first.
    //   t=20 $finish.
    //
    // Expected stdout (exactly):
    //   c2 at 5000
    //   c1 at 5000

    let (stdout, _warnings, _model) =
        run_sim(sv, "tb", "handshake").expect("simulation should run");
    assert_eq!(stdout, "c2 at 5000\nc1 at 5000\n");
}

/// (b) Immediate: `wait (1'b1)` is a constant-true condition with no read
/// signals, so the body runs at t=0 without suspending.
#[test]
fn sim_wait_immediate_constant() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    initial begin
        wait (1'b1);
        $display("immediate at %0t", $time);
    end

    initial #10 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  the initial's wait loop evaluates 1'b1 -> true -> breaks
    //        immediately; the display prints at t=0.
    //   t=10 $finish.
    //
    // Expected stdout (exactly):
    //   immediate at 0

    let (stdout, _warnings, _model) =
        run_sim(sv, "tb", "immediate").expect("simulation should run");
    assert_eq!(stdout, "immediate at 0\n");
}

/// (c) Wait-then-body: the assignment inside the wait body must run only after
/// the condition becomes true.
#[test]
fn sim_wait_then_body() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg go = 0;
    reg [7:0] data = 8'h00;

    initial begin
        wait (go);
        data = 8'h2a;
        $display("data=%0h at %0t", data, $time);
    end

    initial begin
        #3 go = 1;
        $display("go set at %0t", $time);
    end

    initial #20 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  consumer: wait(go) with go=0 -> suspends on go.
    //   t=3  producer: go 0->1, prints "go set at 3".  The change wakes the
    //        consumer, which breaks out of the wait loop, assigns data=8'h2a
    //        and prints "data=2a at 3" (the body runs only after t=3).
    //   t=20 $finish.
    //
    // Expected stdout (exactly):
    //   go set at 3000
    //   data=2a at 3000

    let (stdout, _warnings, _model) = run_sim(sv, "tb", "thenbody").expect("simulation should run");
    assert_eq!(stdout, "go set at 3000\ndata=2a at 3000\n");
}

/// (d) Wait in a task: the task is wait-bearing, so it must be inlined at its
/// call site (the wait loop appears inside the caller's coroutine) and must
/// NOT get a standalone C function.
#[test]
fn sim_wait_task_inlined() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg go = 0;

    task wait_for;
        wait (go);
    endtask

    initial begin
        wait_for;
        $display("task wait done at %0t", $time);
    end

    initial begin
        #2 go = 1;
        $display("go set at %0t", $time);
    end

    initial #20 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  the caller's initial inlines wait_for: the wait loop on go runs
    //        inside the caller's coroutine and suspends (go=0).
    //   t=2  producer: go 0->1, prints "go set at 2".  The caller wakes,
    //        breaks out of the inlined wait loop and prints
    //        "task wait done at 2".
    //   t=20 $finish.
    //
    // Expected stdout (exactly):
    //   go set at 2000
    //   task wait done at 2000

    let (stdout, _warnings, model) = run_sim(sv, "tb", "taskwait").expect("simulation should run");
    assert_eq!(stdout, "go set at 2000\ntask wait done at 2000\n");
    assert!(
        model.contains("if (sv4_to_bool(G_tb_go)) break;"),
        "inlined wait loop not found in generated C"
    );
    assert!(
        !model.contains("fn_tb_wait_for"),
        "wait-bearing task must not become a standalone C function"
    );
}

/// (e) Compound condition: `wait (a && b)` wakes only when both operands are
/// true (a change of `a` alone re-checks and keeps waiting).
#[test]
fn sim_wait_compound_condition() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg a = 0;
    reg b = 0;

    initial begin
        wait (a && b);
        $display("compound at %0t", $time);
    end

    initial begin
        #3 a = 1;
        $display("a set at %0t", $time);
        #4 b = 1;
        $display("b set at %0t", $time);
    end

    initial #30 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  wait(a && b): a=0, b=0 -> false; suspends on the read set {a, b}.
    //   t=3  producer: a 0->1, prints "a set at 3".  The change wakes the
    //        waiter, which re-evaluates a && b = 1 && 0 = false and waits
    //        again on {a, b}.
    //   t=7  producer: b 0->1, prints "b set at 7".  The waiter wakes,
    //        re-evaluates a && b = 1 && 1 = true and prints "compound at 7".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   a set at 3000
    //   b set at 7000
    //   compound at 7000

    let (stdout, _warnings, _model) = run_sim(sv, "tb", "compound").expect("simulation should run");
    assert_eq!(stdout, "a set at 3000\nb set at 7000\ncompound at 7000\n");
}
