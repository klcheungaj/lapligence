//! End-to-end fork/join simulator tests: Surelog compile → codegen → C
//! compile → run, asserting the exact stdout against hand-simulated traces.
//!
//! Surelog writes `slpp_all/` into the process working directory, so the
//! tests run with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, like the other Surelog integration tests).

use std::sync::Mutex;

#[path = "support/sim.rs"]
mod sim_harness;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Compile `sv` (top module `top`), codegen, compile the model with the
/// runtime + libaco and run it; returns the captured stdout.  Each call uses
/// its own temp dir and restores the CWD afterwards.
fn run_design(sv: &str, top: &str, dir_tag: &str) -> Result<String, String> {
    sim_harness::run_sim(sv, top, dir_tag)
}

/// `fork … join`: two branches (`#5` / `#10`) writing signals; the parent
/// joins and only then $displays.  The #5 branch's write + display must come
/// before the #10 branch's, and the parent's display after both.
const JOIN_SV: &str = r#"module tb;
    reg [7:0] a, b;
    initial begin
        fork
            begin #5 a = 8'd1; $display("branch a at t=%0t a=%0d", $time, a); end
            begin #10 b = 8'd2; $display("branch b at t=%0t b=%0d", $time, b); end
        join
        $display("parent after join at t=%0t a=%0d b=%0d", $time, a, b);
        $finish;
    end
endmodule
"#;

// Hand-simulation (the only spawned process is the initial; its fork children
// run on the scheduler like any other coroutine):
//
//   t=0  initial: forks child b0 (#5 a=1) and child b1 (#10 b=2), both
//        enqueued ready; `join` suspends the parent (W_FORK on the group).
//        b0 runs: waits #5.  b1 runs: waits #10.
//   t=5  b0 wakes: a=1 (blocking), $display("branch a at t=5 a=1"),
//        proc_done -> remaining 2 -> 1 (join not complete yet).
//   t=10 b1 wakes: b=2, $display("branch b at t=10 b=2"), proc_done ->
//        remaining 0 -> parent woken.
//        parent: $display("parent after join at t=10 a=1 b=2"); $finish.
//
// Expected stdout (exactly):
//   branch a at t=5 a=1
//   branch b at t=10 b=2
//   parent after join at t=10 a=1 b=2

#[test]
fn fork_join_waits_for_all_children() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_design(JOIN_SV, "tb", "join").expect("simulation should run");
    assert_eq!(
        stdout,
        "branch a at t=5 a=1\nbranch b at t=10 b=2\nparent after join at t=10 a=1 b=2\n"
    );
}

/// `join_any`: the parent resumes after the FIRST branch completes; a later
/// `wait fork;` blocks until both branches have written their signals.
const JOIN_ANY_SV: &str = r#"module tb;
    reg [7:0] a, b;
    initial begin
        fork
            begin #10 a = 8'd1; end
            begin #20 b = 8'd2; end
        join_any
        $display("first branch done at t=%0t a=%0d", $time, a);
        wait fork;
        $display("both branches done at t=%0t a=%0d b=%0d", $time, a, b);
        $finish;
    end
endmodule
"#;

// Hand-simulation:
//
//   t=0  initial forks b0 (#10 a=1) and b1 (#20 b=2); `join_any` suspends
//        the parent.  b0 waits #10, b1 waits #20.
//   t=10 b0 wakes: a=1, proc_done -> first completion wakes the parent
//        (the group stays live: b1 is still pending).
//        parent: $display("first branch done at t=10 a=1"); `wait fork;`
//        (group still live) suspends (W_FORK_ALL).
//   t=20 b1 wakes: b=2, proc_done -> group done -> parent woken.
//        parent: $display("both branches done at t=20 a=1 b=2"); $finish.
//
// Expected stdout (exactly):
//   first branch done at t=10 a=1
//   both branches done at t=20 a=1 b=2

#[test]
fn fork_join_any_resumes_on_first_branch() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_design(JOIN_ANY_SV, "tb", "joinany").expect("simulation should run");
    assert_eq!(
        stdout,
        "first branch done at t=10 a=1\nboth branches done at t=20 a=1 b=2\n"
    );
}

/// `join_none` + `wait fork`: the parent continues immediately after forking,
/// then `wait fork;` blocks until both branches are done.
const JOIN_NONE_WAIT_SV: &str = r#"module tb;
    reg [7:0] a, b;
    initial begin
        fork
            begin #5 a = 8'd1; end
            begin #10 b = 8'd2; end
        join_none
        $display("after join_none at t=%0t", $time);
        wait fork;
        $display("after wait fork at t=%0t a=%0d b=%0d", $time, a, b);
        $finish;
    end
endmodule
"#;

// Hand-simulation:
//
//   t=0  initial forks b0 (#5 a=1) and b1 (#10 b=2); `join_none` returns
//        immediately; $display("after join_none at t=0"); `wait fork;`
//        suspends the parent (its group is still live).
//        b0 waits #5, b1 waits #10.
//   t=5  b0 wakes: a=1, proc_done (group stays live: b1 pending).
//   t=10 b1 wakes: b=2, proc_done -> group done -> parent woken.
//        parent: $display("after wait fork at t=10 a=1 b=2"); $finish.
//
// Expected stdout (exactly):
//   after join_none at t=0
//   after wait fork at t=10 a=1 b=2

#[test]
fn fork_join_none_then_wait_fork() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_design(JOIN_NONE_WAIT_SV, "tb", "joinnone").expect("simulation should run");
    assert_eq!(
        stdout,
        "after join_none at t=0\nafter wait fork at t=10 a=1 b=2\n"
    );
}

/// `disable fork`: a branch records a non-blocking assignment and then waits
/// on a long delay; the parent wakes the branch's runner, kills it with
/// `disable fork;` before the NBA region — the recorded NBA is discarded, so
/// the signal must remain X and the simulation must terminate cleanly.
const DISABLE_FORK_SV: &str = r#"module tb;
    reg [7:0] a;
    reg ev;
    initial begin
        fork
            begin
                a <= 8'd7;
                ev = 1;
                #10 a <= 8'd8;
            end
        join_none
        @(posedge ev);
        disable fork;
        $display("after disable at t=%0t a=%0d", $time, a);
        $finish;
    end
endmodule
"#;

// Hand-simulation:
//
//   t=0  initial forks one child (enqueued ready); `join_none` returns.
//        parent registers @(posedge ev) with last-seen ev=X and suspends.
//        child runs (same active pass): records NBA a<=7, writes ev=1
//        (X->1 = posedge, wakes the parent), then waits #10.
//        parent runs (same pass): `disable fork;` kills the child while it
//        is suspended — llg_kill_proc frees the child's pending NBA list,
//        so a<=7 is discarded and never committed (the child's all_procs
//        slot is NULLed).  $display prints a=x, $finish.
//
// Expected stdout (exactly):
//   after disable at t=0 a=x

#[test]
fn disable_fork_discards_killed_children_nbas() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_design(DISABLE_FORK_SV, "tb", "disable").expect("simulation should run");
    assert_eq!(stdout, "after disable at t=0 a=x\n");
}

/// A `fork … join` inside a `for` loop: three iterations, each forking a
/// single `#1` branch that increments `acc`; the join makes each iteration
/// wait for its branch before the next fork.
const FORK_IN_FOR_SV: &str = r#"module tb;
    integer i;
    reg [3:0] acc;
    initial begin
        acc = 0;
        for (i = 0; i < 3; i = i + 1) begin
            fork
                #1 acc = acc + 1;
            join
        end
        $display("acc=%0d at t=%0t", acc, $time);
        $finish;
    end
endmodule
"#;

// Hand-simulation:
//
//   t=0  acc=0.  Iteration 0: fork child0 (#1), join -> parent suspends;
//        child0 waits #1.
//   t=1  child0: acc=1 (blocking), done -> parent woken.
//        Iteration 1: fork child1, join; child1 waits #1.
//   t=2  child1: acc=2, done.  Iteration 2: fork child2, join; child2 waits #1.
//   t=3  child2: acc=3, done.  Parent: $display("acc=3 at t=3"); $finish.
//
// Expected stdout (exactly):
//   acc=3 at t=3

#[test]
fn fork_join_inside_for_loop() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_design(FORK_IN_FOR_SV, "tb", "for").expect("simulation should run");
    assert_eq!(stdout, "acc=3 at t=3\n");
}
