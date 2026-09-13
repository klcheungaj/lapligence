//! Simulation-region conformance tests (IEEE 1800-2017 §4 scheduling regions).
//!
//! Each test compiles a small design (Slang → semantic DB → codegen → C11 via CMake),
//! runs the simulator executable and asserts the exact stdout against a
//! hand-simulated trace of the region loop:
//!
//!   active region (FIFO ready queue) → inactive region (`#0` waiters, drained
//!   in a loop) → NBA region (commit per-process lists) → repeat while new
//!   events appeared → advance time.
//!
//! `#0` delays resume in the INACTIVE region, which runs BETWEEN the active
//! region and the NBA region (LRM §4.4.2): a `#0` continuation reads the
//! pre-NBA values of the same time step.
//!
//! These tests temporarily change the process working directory, so every
//! test runs with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, to avoid process-wide CWD races).

use std::process::Command;
use std::sync::Mutex;
use std::time::Duration;

use llg::core::compile;
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Compile `sv` (top module `tb`), codegen, compile the model with the
/// runtime + libaco and run it; returns `(stdout, stderr)`.  Each call uses
/// its own temp dir and restores the CWD afterwards.
fn run_design(sv: &str, tag: &str) -> Result<(String, String), String> {
    sim_harness::with_temp_cwd(tag, |dir| {
        let src = dir.join("region.sv");
        std::fs::write(&src, sv).map_err(|e| format!("write source: {e}"))?;
        let out = compile::compile(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let db =
            llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
        let gen = sim::codegen::generate(&db).map_err(|e| format!("codegen: {e}"))?;
        let exe = sim::build::build_model_cmake(dir, &[("model.c", gen.model_c.as_str())])
            .map_err(|e| format!("cmake: {e}"))?;
        let output = sim_harness::run_command(&mut Command::new(&exe), Duration::from_secs(60))?;
        if !output.status.success() {
            return Err(format!(
                "sim exited with {:?}, stderr: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok((
            String::from_utf8_lossy(&output.stdout).into_owned(),
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ))
    })
}

/// 1. NBA semantics: RHS sampled when the NBA statement executes; LHS updated
///    in the NBA region.  A blocking read in the same process (and in a second
///    process in the same active pass) must see the OLD value; a process woken
///    by the NBA commit must see the NEW value.
const NBA_VISIBILITY_SV: &str = r#"`timescale 1ns/1ns
module tb;
    reg a = 1'b0;
    reg b, c, d;

    // NBA + blocking read in the same process: b/c sample the OLD a.
    initial begin
        a <= 1'b1;
        b = a;
        c = a;
    end

    // Second process reading a in the same active pass: still old.
    initial begin
        d = a;
    end

    // Third process: woken only by the NBA commit; sees the new a.
    always @(posedge a) begin
        $display("after NBA: a=%0d b=%0d c=%0d d=%0d", a, b, c, d);
        #1 $display("settled: a=%0d b=%0d c=%0d d=%0d", a, b, c, d);
        $finish;
    end
endmodule
"#;

// Hand-simulation (timescale 1ns/1ns → 1 tick = 1 ns):
//
//   t=0  declaration initializer: a=0 (applied in main() before any process).
//        Spawn order (db child order): initial#1, initial#2, always@(posedge a).
//        Active pass (FIFO):
//          initial#1: records NBA a<=1 (a still 0); b=a -> b=0 (blocking read
//                     samples the pre-NBA value); c=a -> c=0.
//          initial#2: d=a -> d=0 (same active pass; NBA not committed yet).
//          always@(posedge a): registers a posedge waiter with last-seen a=0.
//        NBA region: commits a=1; 0->1 posedge wakes the always.
//        Active pass (same time step): always: $display("after NBA: a=1 b=0
//        c=0 d=0"); #1 -> t=1.
//   t=1  always: $display("settled: a=1 b=0 c=0 d=0"); $finish.
//
// Expected stdout (exactly):
//   after NBA: a=1 b=0 c=0 d=0
//   settled: a=1 b=0 c=0 d=0

#[test]
fn region_nba_visibility() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let (stdout, _stderr) = run_design(NBA_VISIBILITY_SV, "nba").expect("simulation should run");
    assert_eq!(
        stdout,
        "after NBA: a=1 b=0 c=0 d=0\nsettled: a=1 b=0 c=0 d=0\n"
    );
}

/// NBA updates issued by one process retain source issue order at commit. The
/// final write wins without relying on any ordering between racing processes.
const NBA_ISSUE_ORDER_SV: &str = r#"`timescale 1ns/1ns
module tb;
    reg [7:0] a;
    initial begin
        a <= 8'd1;
        a <= 8'd2;
        a <= 8'd3;
        #1 $display("a=%0d", a);
        $finish;
    end
endmodule
"#;

#[test]
fn region_nba_issue_order_within_process() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let (stdout, _stderr) =
        run_design(NBA_ISSUE_ORDER_SV, "nba_issue_order").expect("simulation should run");
    assert_eq!(stdout, "a=3\n");
}

/// 2. `#0` must move the continuation to the INACTIVE region, which runs
///    BETWEEN the active region and the NBA region.  A `#0` process must
///    therefore read the PRE-NBA value of a signal NBA-assigned earlier in the
///    same time step.
const ZERO_DELAY_INACTIVE_SV: &str = r#"`timescale 1ns/1ns
module tb;
    reg a = 1'b0;
    initial begin
        a <= 1'b1;
        #0 $display("a=%0d", a);
        $finish;
    end
endmodule
"#;

// Hand-simulation (LRM):
//   t=0 active:   records NBA a<=1 (a still 0); #0 -> INACTIVE region.
//   t=0 inactive: resumes; $display reads a=0 (NBA not yet committed);
//                 $finish.
//   Expected stdout: a=0

#[test]
fn region_zero_delay_inactive() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let (stdout, _stderr) =
        run_design(ZERO_DELAY_INACTIVE_SV, "zero_inactive").expect("simulation should run");
    assert_eq!(stdout, "a=0\n");
}

/// 3. Multi-process variant of the `#0`-vs-NBA ordering: the `#0` process
///    (second initial) runs after the first initial in the same time step, so it
///    must see the first initial's NBAs only if the NBA region has already
///    committed them.  LRM says the inactive region (and therefore the `#0`
///    process) runs BEFORE the NBA region.
const ZERO_DELAY_MULTI_PROC_SV: &str = r#"`timescale 1ns/1ns
module tb;
    reg a = 1'b0, b = 1'b0;
    initial begin
        a <= 1'b1;
        b <= 1'b1;
    end
    initial begin
        #0 $display("a=%0d b=%0d", a, b);
        $finish;
    end
endmodule
"#;

// Hand-simulation (LRM):
//   t=0 active:   initial#1 records NBA a<=1, b<=1 (a, b still 0);
//                 initial#2 executes #0 -> INACTIVE region.
//   t=0 inactive: initial#2 resumes; $display reads a=0 b=0; $finish.
//   Expected stdout: a=0 b=0

#[test]
fn region_zero_delay_multi_proc() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let (stdout, _stderr) =
        run_design(ZERO_DELAY_MULTI_PROC_SV, "zero_multi").expect("simulation should run");
    assert_eq!(stdout, "a=0 b=0\n");
}

/// 4. NBA commits re-trigger the active region until the design quiesces:
///    a flop plus two comb stages chained through NBAs settles over three NBA
///    commits in one time step, all before time advances.
///
/// Note: the original spec sketch used three `always @(posedge clk)` blocks
/// in a ring; those processes are sensitive only to `clk`, so a posedge would
/// settle in a single NBA commit and would NOT exercise the re-run loop.  A
/// flop + two `always_comb` stages is used instead: the posedge samples the
/// pre-charged comb output, and each NBA commit wakes the next comb stage
/// within the same time step (3 commits at t=1).
const MULTI_DELTA_SETTLE_SV: &str = r#"`timescale 1ns/1ns
module tb;
    reg clk;
    reg [3:0] q, w, d;

    // Flop: samples the comb chain on each posedge.
    always @(posedge clk) q <= d;
    // Two comb stages chained through NBA commits.
    always_comb w <= q + 4'd1;
    always_comb d <= w + 4'd1;

    initial begin
        clk = 0;
        q = 4'd0;         // blocking: kicks the comb chain at t=0
        #1 $display("pre posedge: q=%0d w=%0d d=%0d", q, w, d);
        clk = 1;          // posedge at t=1
        #1 $display("post posedge: q=%0d w=%0d d=%0d", q, w, d);
        $finish;
    end
endmodule
"#;

// Hand-simulation (timescale 1ns/1ns → 1 tick = 1 ns):
//
//   t=0  spawn order: always@(posedge clk) flop, always_comb w, always_comb d,
//        initial.  All signals X.
//        flop registers @(posedge clk) with last-seen clk=X.
//        always_comb w evaluates once: w <= q+1 = X (NBA); waits on {q} with
//        last-seen q=X.
//        always_comb d evaluates once: d <= w+1 = X (NBA); waits on {w} with
//        last-seen w=X.
//        initial: clk=0 (X->0, not a posedge); q=0 (X->0 wakes w); #1 -> t=1.
//        w (woken): w <= q+1 = 1; waits on {q} (last-seen q=0).
//        NBA region: commits w=X (initial eval) then w=1; X->1 wakes d.
//        d (woken): d <= w+1 = 2; waits on {w} (last-seen w=1).
//        NBA region: commits d=X then d=2; X->2.  Quiescent.
//   t=1  initial: $display("pre posedge: q=0 w=1 d=2"); clk=1 (0->1 POSEDGE
//        wakes the flop); #1 -> t=2.
//        flop: q <= d = 2.  NBA region commits q=2 (0->2 wakes w).
//        w:    w <= q+1 = 3.  NBA region commits w=3 (1->3 wakes d).
//        d:    d <= w+1 = 4.  NBA region commits d=4 (2->4).  Quiescent.
//        The q, w and d NBA commits all happen in the SAME time step (t=1):
//        each commit wakes the next comb process, which records another NBA
//        committed by the next NBA-region pass before time advances.
//   t=2  initial: $display("post posedge: q=2 w=3 d=4"); $finish.
//
// Expected stdout (exactly):
//   pre posedge: q=0 w=1 d=2
//   post posedge: q=2 w=3 d=4

#[test]
fn region_multi_delta_settle() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let (stdout, _stderr) =
        run_design(MULTI_DELTA_SETTLE_SV, "delta").expect("simulation should run");
    assert_eq!(
        stdout,
        "pre posedge: q=0 w=1 d=2\npost posedge: q=2 w=3 d=4\n"
    );
}

/// 5. Blocking writes in one active pass are FIFO in process spawn order:
///    two initials writing the same signal, each displaying immediately after
///    its own write, plus a `#1` observer showing last-write-wins.
const BLOCKING_ORDER_FIFO_SV: &str = r#"`timescale 1ns/1ns
module tb;
    reg a;
    initial begin
        a = 1'b1;
        $display("after p1: a=%0d", a);
    end
    initial begin
        a = 1'b0;
        $display("after p2: a=%0d", a);
    end
    initial begin
        #1 $display("final: a=%0d", a);
        $finish;
    end
endmodule
"#;

// Hand-simulation (timescale 1ns/1ns):
//
//   t=0  spawn order (db child order): initial#1, initial#2, initial#3.
//        Active pass (FIFO ready queue == spawn order):
//          initial#1: a=1 (blocking); $display("after p1: a=1").
//          initial#2: a=0 (blocking); $display("after p2: a=0").
//          initial#3: #1 -> t=1.
//        Blocking writes land in the ACTIVE region immediately, so each
//        initial's display sees its own write; the last write (initial#2)
//        wins.
//   t=1  initial#3: $display("final: a=0"); $finish.
//
// Expected stdout (exactly):
//   after p1: a=1
//   after p2: a=0
//   final: a=0

#[test]
fn region_blocking_order_fifo() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let (stdout, _stderr) =
        run_design(BLOCKING_ORDER_FIFO_SV, "fifo").expect("simulation should run");
    assert_eq!(stdout, "after p1: a=1\nafter p2: a=0\nfinal: a=0\n");
}

/// 6. An edge waiter on a signal driven by an NBA in the same time step must
///    see the edge only AFTER the NBA region commits: the writer's own display
///    (same active pass as the NBA record) reads the pre-NBA value, and the
///    waiter's display (same time step, post-commit) reads the new value.
const WAIT_EDGE_NBA_COMMIT_SV: &str = r#"`timescale 1ns/1ns
module tb;
    reg clk;
    always @(posedge clk) begin
        $display("posedge seen: t=%0t clk=%0d", $time, clk);
        $finish;
    end
    initial begin
        clk = 1'b0;
        #1 begin
            clk <= 1'b1;                 // NBA: clk still 0 in this active pass
            $display("NBA recorded: t=%0t clk=%0d", $time, clk);
        end
        #1 $display("after: t=%0t clk=%0d", $time, clk);
        $finish;
    end
endmodule
"#;

// Hand-simulation (timescale 1ns/1ns → 1 tick = 1 ns):
//
//   t=0  always@(posedge clk) registers with last-seen clk=X.
//        initial: clk=0 (X->0 is not a posedge; the always watches posedge
//        only), #1 -> t=1.
//   t=1  initial: records NBA clk<=1 (clk still 0 in this active pass);
//        $display("NBA recorded: t=1 clk=0"); #1 -> t=2.
//        NBA region: commits clk=1; 0->1 POSEDGE wakes the always.
//        Active pass (same time step t=1): always:
//        $display("posedge seen: t=1 clk=1"); $finish.
//        The "after" display at t=2 never runs ($finish fires first).
//
// Expected stdout (exactly):
//   NBA recorded: t=1 clk=0
//   posedge seen: t=1 clk=1

#[test]
fn region_wait_edge_nba_commit() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let (stdout, _stderr) =
        run_design(WAIT_EDGE_NBA_COMMIT_SV, "edge").expect("simulation should run");
    assert_eq!(stdout, "NBA recorded: t=1 clk=0\nposedge seen: t=1 clk=1\n");
}

/// 7. An infinite zero-delay loop (`always begin #0; end`) must be stopped by
///    the runtime's zero-loop guard (LLG_ZERO_LOOP_LIMIT region passes within
///    one time step), not hang the simulation.  The runtime prints
///    "llg: zero-delay loop detected at time 0" to stderr, breaks out of the
///    region loop and main returns a controlled nonzero status.
const ZERO_DELAY_LOOP_GUARD_SV: &str = r#"`timescale 1ns/1ns
module tb;
    always begin
        #0;
    end
endmodule
"#;

// Hand-simulation:
//   t=0  active pass: the always records a #0 wait on the INACTIVE list.
//        Inactive drain: the waiter is re-woken and re-registers a #0 in the
//        same time step; this repeats until the region-pass counter exceeds
//        LLG_ZERO_LOOP_LIMIT (10,000,000), at which point the runtime prints
//        the guard message and breaks.
//
// Expected stdout: (empty)
// Expected stderr contains: "llg: zero-delay loop detected at time 0"
// Expected exit: failure (1), because the simulation did not converge.

#[test]
fn region_zero_delay_loop_guard() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let run =
        sim_harness::run_generated_sim_allow_failure(ZERO_DELAY_LOOP_GUARD_SV, "tb", "zeroloop")
            .expect("simulation should terminate");
    assert_eq!(run.status.code(), Some(1));
    assert!(run.stdout.is_empty());
    let stderr = run.stderr;
    assert!(
        stderr.contains("llg: zero-delay loop detected at time 0"),
        "stderr did not contain the zero-delay loop guard message: {stderr}"
    );
}

/// 8. Fork/join + delta timing: the parent resumes after `join` in the ACTIVE
///    region, BEFORE the NBA region commits the children's NBAs — so a plain
///    $display right after `join` reads the pre-NBA values.  The children's NBAs
///    are committed before the end of the time step, which a $strobe observes.
///
/// This pins the LRM-correct behavior: fork children's NBAs are NOT visible
/// to the parent immediately after `join`; they become visible only through a
/// post-NBA read (here $strobe).
const FORK_JOIN_DELTA_SV: &str = r#"`timescale 1ns/1ns
module tb;
    reg [7:0] a = 8'd0;
    reg [7:0] b = 8'd0;
    initial begin
        fork
            begin a <= 8'd1; end
            begin b <= 8'd2; end
        join
        $display("after join (active): a=%0d b=%0d", a, b);
        $strobe("after join (strobe): a=%0d b=%0d", a, b);
        // A new time slot lets the NBA and Postponed regions complete.
        #1 $finish(0);
    end
endmodule
"#;

// Hand-simulation (timescale 1ns/1ns):
//
//   t=0  declaration initializers: a=0, b=0.
//        initial forks child c1 (a<=1) and c2 (b<=2); `join` suspends the
//        parent (remaining=2).
//        Active pass:
//          c1: records NBA a<=1; proc_done (remaining 2->1).
//          c2: records NBA b<=2; proc_done (remaining 1->0 -> parent woken).
//          parent resumes in the SAME active pass, BEFORE the NBA region:
//          $display reads a=0 b=0 (the children's NBAs are still pending);
//          $strobe queued; parent suspends for one time unit.
//        NBA region: commits a=1, b=2.
//        $strobe flush: prints with the committed values a=1 b=2.
//   t=1  parent resumes and finishes; no pending t=0 work is discarded.
//
// Expected stdout (exactly):
//   after join (active): a=0 b=0
//   after join (strobe): a=1 b=2

#[test]
fn region_fork_join_delta() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let (stdout, _stderr) =
        run_design(FORK_JOIN_DELTA_SV, "forkjoin").expect("simulation should run");
    assert_eq!(
        stdout,
        "after join (active): a=0 b=0\nafter join (strobe): a=1 b=2\n"
    );
}
