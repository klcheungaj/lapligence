//! End-to-end simulator tests for named events (`event ev;` / `-> ev;` /
//! `@(ev)`): Surelog compile → codegen → CMake build → run, asserting exact
//! stdout against hand-simulated traces.
//!
//! Regression coverage: producer/consumer handshake, all-current-waiters
//! wake semantics (each waiter woken exactly once, registration order),
//! mixed signal/event or-lists lowered as ONE atomic wait, edge-triggered
//! (non-latching) event semantics, the zero-delay guard tripping on trigger
//! loops, generate-scope events, and the clean rejections (edge control on
//! an event; event arrays and hierarchical event references are rejected by
//! the Surelog frontend itself).
//!
//! Surelog writes `slpp_all/` into the process working directory, so the
//! tests run with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, like the other Surelog integration tests).

#[path = "support/sim.rs"]
mod sim_harness;

use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Compile, generate, build, and run one design.
fn run_sim(sv: &str, top: &str, tag: &str) -> Result<(String, Vec<String>, String), String> {
    let run = sim_harness::run_generated_sim(sv, top, tag)?;
    Ok((run.stdout, run.warnings, run.model_c))
}

/// Compile + codegen only (no model build): the codegen error message, when
/// the design must be rejected at lowering.  Returns `Err` when Surelog
/// itself rejects the design (the message then starts with "COMPILE-ERROR:").
fn codegen_error(sv: &str, top: &str, tag: &str) -> Result<String, String> {
    sim_harness::with_temp_cwd(tag, |dir| {
        let src = dir.join("tb.sv");
        std::fs::write(&src, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some(top.to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!(
                "COMPILE-ERROR: {}",
                out.diagnostics
                    .iter()
                    .map(|d| d.message.clone())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        match sim::codegen::generate(design) {
            Ok(_) => Err("design was expected to be rejected".to_string()),
            Err(e) => Ok(e.to_string()),
        }
    })
}

/// (a) Handshake: a producer triggers `ev` at t=5 while two consumers wait on
/// it.  Every current waiter wakes on the trigger.
#[test]
fn sim_events_handshake() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    event ev;

    initial begin
        @(ev);
        $display("c1 woken at %0t", $time);
    end

    initial begin
        @(ev);
        $display("c2 woken at %0t", $time);
    end

    initial begin
        #5 -> ev;
        $display("triggered at %0t", $time);
    end

    initial #20 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  both consumers register on ev's waiter table (c1 first, then
    //        c2, matching the process spawn order); the producer delays #5.
    //   t=5  producer executes `-> ev`: every current waiter is detached and
    //        scheduled in registration order, so c1 resumes before c2 and
    //        both print at t=5.  The producer's own display follows (it ran
    //        first in the active region).
    //   t=20 $finish.
    //
    // Expected stdout (exactly):
    //   triggered at 5
    //   c1 woken at 5
    //   c2 woken at 5

    let (stdout, _warnings, _model) =
        run_sim(sv, "tb", "handshake").expect("simulation should run");
    assert_eq!(stdout, "triggered at 5\nc1 woken at 5\nc2 woken at 5\n");
}

/// (b) Multiple waiters are woken EXACTLY once each by one trigger: a third
/// consumer joins the wait list and every consumer counts its wakeups.
#[test]
fn sim_events_multiple_waiters_woken_once() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    event ev;
    integer c1_count = 0;
    integer c2_count = 0;
    integer c3_count = 0;

    initial begin
        @(ev); c1_count = c1_count + 1;
        @(ev); c1_count = c1_count + 1;
    end

    initial begin
        @(ev); c2_count = c2_count + 1;
        @(ev); c2_count = c2_count + 1;
    end

    initial begin
        @(ev); c3_count = c3_count + 1;
        @(ev); c3_count = c3_count + 1;
    end

    initial begin
        #3 -> ev;
        #7 -> ev;
        #2 $display("counts: %0d %0d %0d", c1_count, c2_count, c3_count);
    end

    initial #30 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  c1/c2/c3 register (spawn order c1 < c2 < c3).
    //   t=3  trigger: all three detach and print-count once each; they then
    //        re-register.  Wake order is registration order, but each
    //        increments only its own counter, so the final totals pin the
    //        "exactly once per waiter" contract regardless of ordering.
    //   t=10 trigger: again all three count their second wakeup.
    //   t=12 producer displays the counters.
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   counts: 2 2 2

    let (stdout, _warnings, _model) =
        run_sim(sv, "tb", "multiwaiter").expect("simulation should run");
    assert_eq!(stdout, "counts: 2 2 2\n");
}

/// (c) Mixed or-list `@(a or ev)` wakes on EITHER source, as ONE atomic wait:
/// the trigger cannot be lost between two separate sub-waits.  The generated
/// C must contain EXACTLY ONE `llg_wait_mixed` call per `@(a or ev)`
/// statement (this design has two, plus exactly one trigger call) — a
/// presence-only check could not catch a lowering that splits one site into
/// several sequential sub-waits.
#[test]
fn sim_events_mixed_list_atomic() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    event ev;
    reg a = 0;

    initial begin
        @(a or ev);
        $display("first wake (%b) at %0t", a, $time);
        @(a or ev);
        $display("second wake (%b) at %0t", a, $time);
    end

    initial begin
        #4 -> ev;
        #4 a = 1;
    end

    initial #20 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  consumer registers ONE wait covering both `a` (any change) and
    //        `ev` (trigger).
    //   t=4  producer triggers ev: the consumer wakes with a still 0 and
    //        prints "first wake (0) at 4", then re-registers both sources.
    //   t=8  producer sets a=1: the change wakes the consumer, which prints
    //        "second wake (1) at 8".
    //   t=20 $finish.
    //
    // Expected stdout (exactly):
    //   first wake (0) at 4
    //   second wake (1) at 8

    let (stdout, _warnings, model) = run_sim(sv, "tb", "mixed").expect("simulation should run");
    assert_eq!(stdout, "first wake (0) at 4\nsecond wake (1) at 8\n");
    assert_eq!(
        model.matches("llg_wait_mixed(src, 2);").count(),
        2,
        "each of the two @(a or ev) statements must lower to EXACTLY ONE \
         atomic llg_wait_mixed call (never a split into sub-waits)"
    );
    assert_eq!(
        model.matches("llg_event_trigger(&E_tb_ev);").count(),
        1,
        "the trigger statement must call llg_event_trigger on the event \
         global exactly once"
    );
}

/// An event-only `or` list is one atomic wait over both named events.  After
/// the first event wakes the process, the second wait must be registered on
/// both events again without leaving a stale registration behind.
#[test]
fn sim_events_event_only_or_list_rearms_on_each_event() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    event ev1, ev2;

    initial begin
        @(ev1 or ev2);
        $display("wake one at %0t", $time);
        @(ev1 or ev2);
        $display("wake two at %0t", $time);
    end

    initial begin
        #3 -> ev2;
        $display("trigger ev2 at %0t", $time);
        #2 -> ev1;
        $display("trigger ev1 at %0t", $time);
    end

    initial #10 $finish;
endmodule
"#;

    let (stdout, _warnings, _model) = run_sim(sv, "tb", "event-or").expect("simulation should run");
    assert_eq!(
        stdout,
        "trigger ev2 at 3\nwake one at 3\ntrigger ev1 at 5\nwake two at 5\n"
    );
}

/// (d) Events are edge-triggered, not stateful: a trigger fired BEFORE a
/// waiter registers does NOT latch — the late waiter keeps waiting.
#[test]
fn sim_events_trigger_before_wait_does_not_latch() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    event ev;

    initial begin
        #2 -> ev;
        $display("triggered at %0t", $time);
    end

    initial begin
        #5 @(ev);
        $display("WRONG: latched at %0t", $time);
    end

    initial #10 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=2  producer triggers ev: no waiters are registered, so the trigger
    //        is simply lost.
    //   t=5  the second process registers on ev — nothing wakes it (events
    //        carry no value; there is nothing to observe).
    //   t=10 $finish ends the simulation while the waiter is suspended.
    //
    // Expected stdout (exactly):
    //   triggered at 2

    let (stdout, _warnings, _model) = run_sim(sv, "tb", "nolatch").expect("simulation should run");
    assert_eq!(stdout, "triggered at 2\n");
}

/// (e) A zero-delay trigger loop (`#0 -> ev` ping-ponged between two events)
/// must be stopped by the runtime's zero-delay guard, printing
/// "llg: zero-delay loop detected at time 0" — never hanging.
#[test]
fn sim_events_zero_delay_trigger_loop_guard() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    event ev;
    event ev2;

    initial forever begin #0 -> ev; end
    initial forever begin @(ev) -> ev2; end
    initial forever begin @(ev2) -> ev; end

    initial #1000000 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  the three processes form a trigger cycle: each #0 pass makes
    //        one process trigger an event whose waiter immediately triggers
    //        the next event, so the region loop never reaches quiescence.
    //        The scheduler's per-resume region-pass counter exceeds
    //        LLG_ZERO_LOOP_LIMIT, the runtime prints the guard message to
    //        stderr and stops (exit success, empty stdout).
    //
    // Expected stdout: (empty)
    // Expected stderr contains: "llg: zero-delay loop detected at time 0"

    let stderr = sim_harness::run_generated_sim(sv, "tb", "events-guard")
        .expect("simulation should terminate via the guard")
        .stderr;
    assert!(
        stderr.contains("llg: zero-delay loop detected at time 0"),
        "stderr did not contain the zero-delay loop guard message: {stderr}"
    );
}

/// (f) Generate-scope events: each iteration gets its OWN event; triggering
/// one instance's event does not wake another iteration's waiter.
#[test]
fn sim_events_gen_scope_instances() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    genvar i;
    generate
        for (i = 0; i < 2; i = i + 1) begin : g
            event gev;
            initial begin
                @(gev);
                $display("gen %0d woken at %0t", i, $time);
            end
            initial begin
                #3 -> gev;
            end
        end
    endgenerate
    initial #20 $finish;
endmodule
"#;

    // Hand-simulation:
    //   t=0  per iteration i: a waiter registers on g[i].gev (a distinct
    //        event global per elaborated scope).
    //   t=3  each iteration's trigger fires its own event; both waiters
    //        wake at t=3, iteration 0 first (spawn order).
    //   t=20 $finish.
    //
    // Expected stdout (exactly):
    //   gen 0 woken at 3
    //   gen 1 woken at 3

    let (stdout, _warnings, _model) = run_sim(sv, "tb", "genscope").expect("simulation should run");
    assert_eq!(stdout, "gen 0 woken at 3\ngen 1 woken at 3\n");
}

/// (g) Rejections: `@(posedge ev)` is a clean codegen error; event arrays and
/// hierarchical event references never reach the simulator (Surelog rejects
/// them at compile time).
#[test]
fn sim_events_rejections() {
    let _guard = SURELOG_LOCK.lock().unwrap();

    // Edge control on a named event: no value means no edges.
    let err = codegen_error(
        r#"module tb;
    event ev;
    initial begin
        @(posedge ev);
    end
endmodule
"#,
        "tb",
        "reject-edge",
    )
    .expect("design should produce a codegen error");
    assert!(
        err.contains("edge control on a named event is not supported in v1"),
        "unexpected error: {err}"
    );

    // Event arrays: Surelog v1.86's grammar cannot even parse them.
    let err = match codegen_error(
        r#"module tb;
    event ev[4];
    initial begin
        -> ev[0];
    end
endmodule
"#,
        "tb",
        "reject-array",
    ) {
        Err(e) => e,
        Ok(other) => panic!("expected the Surelog syntax reject, got: {other}"),
    };
    assert!(
        err.contains("COMPILE-ERROR") && err.contains("Syntax error"),
        "expected the Surelog syntax reject, got: {err}"
    );

    // Hierarchical event references: rejected by Surelog elaboration.
    let err = match codegen_error(
        r#"module sub;
    event sev;
endmodule
module tb;
    sub u0();
    initial begin
        @(u0.sev);
    end
endmodule
"#,
        "tb",
        "reject-hier",
    ) {
        Err(e) => e,
        Ok(other) => panic!("expected the Surelog hierarchical-reference reject, got: {other}"),
    };
    assert!(
        err.contains("Unresolved hierarchical reference"),
        "expected the Surelog hierarchical-reference reject, got: {err}"
    );
}

/// (h) Optimizer parity: a design mixing events, signals and dead storage
/// produces byte-identical stdout with and without optimization passes.
#[test]
fn sim_events_opt_parity() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    use llg::sim::opt::OptConfig;

    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    event ev;
    reg a = 0;
    reg [7:0] data = 8'h00;
    reg [7:0] dead = 8'hFF;

    initial begin
        @(a or ev);
        data = 8'h2a;
        $display("data=%0h at %0t", data, $time);
    end

    initial begin
        #5 -> ev;
    end

    initial begin
        #9 a = 1'b1;
        $display("a set at %0t", $time);
    end

    initial #20 $finish;
endmodule
"#;

    // Hand-simulation (both configurations):
    //   t=0  consumer parks on the mixed wait (a, ev); `dead` drives nothing.
    //   t=5  trigger: consumer wakes, data=2a printed.
    //   t=9  a set: display "a set at 9".
    //   t=20 $finish.
    //
    // Expected stdout (exactly):
    //   data=2a at 5
    //   a set at 9

    // House pattern (sim_opt_differential): ONE Surelog compile and owned DB;
    // both configurations generate from that immutable snapshot.
    let dir = sim_harness::TempDir::new("events-optparity").expect("create temp dir");
    let src = dir.path().join("tb.sv");
    std::fs::write(&src, sv).expect("write source");

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
        let opt_on = sim::codegen::generate_from_db_with_opts(&db, &OptConfig::default())
            .map_err(|e| format!("codegen(opt-on): {e}"))?;
        let opt_off = sim::codegen::generate_from_db_with_opts(&db, &OptConfig::none())
            .map_err(|e| format!("codegen(opt-off): {e}"))?;

        let run_variant = |name: &str, model_c: &str| -> Result<String, String> {
            let out_dir = dir.path().join(name);
            let exe = sim::build::build_model_cmake(&out_dir, &[("model.c", model_c)])
                .map_err(|e| format!("cmake({name}): {e}"))?;
            sim_harness::run_executable(&exe).map_err(|error| format!("run({name}): {error}"))
        };
        let on = run_variant("opt_on", &opt_on.model_c)?;
        let off = run_variant("opt_off", &opt_off.model_c)?;
        if on != off {
            return Err(format!(
                "optimization passes changed observable behavior:\n on:  {on:?}\n off: {off:?}"
            ));
        }
        Ok(on)
    });
    let stdout = result.expect("both runs should succeed");
    assert_eq!(stdout, "data=2a at 5\na set at 9\n");
}
