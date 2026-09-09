//! End-to-end simulator tests for `disable <label>;` (1364-1995 §11) and
//! `break` / `continue` and `do … while` (1800-2005 §12.7): Slang compile → codegen → CMake
//! build → run, asserting exact stdout against hand-simulated traces.
//!
//! Covered: disabling an enclosing named begin block mid-loop (statements
//! after the disable are skipped), the classic `for … begin : loop … if
//! (cond) disable loop; end` early-exit idiom, `disable <taskname>;` inside
//! the task as an early return (both the plain C-function compilation and the
//! inlined wait-bearing expansion), break/continue in for/while/repeat/forever
//! with pinned iteration counts, nesting rules (break exits the innermost
//! loop only; disable exits the named level across any nesting), the clean
//! codegen reject for cross-process disables, and optimization parity.
//! Post-test loops are pinned for execute-once behavior and for `continue`
//! evaluating the condition before the next iteration.
//!
//! These tests temporarily change the process working directory, so the
//! tests run with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, to avoid process-wide CWD races).

#[path = "support/sim.rs"]
mod sim_harness;

use std::sync::Mutex;

use llg::core::compile;
use llg::sim;
use llg::sim::opt::OptConfig;

static CWD_LOCK: Mutex<()> = Mutex::new(());

/// Compile, generate, build, and run one design.
fn run_sim(sv: &str, top: &str, tag: &str) -> Result<(String, Vec<String>), String> {
    let run = sim_harness::run_generated_sim(sv, top, tag)?;
    Ok((run.stdout, run.warnings))
}

/// Compile + codegen only, returning the raw codegen error (for rejects).
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
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let db =
            llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
        match sim::codegen::generate(&db) {
            Ok(_) => Err("codegen unexpectedly succeeded".to_string()),
            Err(e) => Ok(e.to_string()),
        }
    })
}

/// Build + run one model under a specific optimizer configuration; returns
/// its exact stdout (used by the opt-parity case).
fn run_variant(dir: &std::path::Path, name: &str, model_c: &str) -> Result<String, String> {
    let out_dir = dir.join(name);
    let exe = sim::build::build_model_cmake(&out_dir, &[("model.c", model_c)])
        .map_err(|e| format!("cmake({name}): {e}"))?;
    sim_harness::run_executable(&exe).map_err(|error| format!("run({name}): {error}"))
}

#[test]
fn do_while_is_post_test_and_honors_loop_control() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"`timescale 1ns/1ps
module tb;
    integer once;
    integer i;
    integer sum;

    initial begin
        once = 0;
        do once = once + 1; while (0);

        i = 0;
        sum = 0;
        do begin
            i = i + 1;
            if (i == 2) continue;
            if (i == 5) break;
            sum = sum + i;
        end while (i < 8);

        $display("once=%0d i=%0d sum=%0d", once, i, sum);
        $finish;
    end
endmodule
"#;

    let (stdout, warnings) = run_sim(sv, "tb", "do_while").expect("simulation should run");
    assert_eq!(stdout, "once=1 i=5 sum=8\n");
    assert!(warnings.is_empty(), "unexpected warnings: {warnings:?}");
}

/// (a) Disable an enclosing NAMED block mid-loop: everything after the
/// `disable blk;` — the rest of the loop AND the rest of the named block —
/// is skipped; execution resumes after the block.
#[test]
fn disable_enclosing_named_block_midloop() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    integer i;
    reg [7:0] acc;

    initial begin : blk
        acc = 0;
        for (i = 0; i < 8; i = i + 1) begin
            if (i == 3) disable blk;
            acc = acc + i;
        end
        $display("after loop");   // must NOT print
    end

    initial begin
        #1 $display("final acc=%0d", acc);
        $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  block blk: acc=0.  Loop: i=0 -> acc=0; i=1 -> acc=1; i=2 ->
    //        acc=3; at i=3 the guard fires BEFORE the accumulate, so
    //        `disable blk` jumps past the end of the whole named block:
    //        iterations i>=3 never accumulate and "after loop" never runs.
    //   t=1  second initial prints acc (=3).
    //
    // Expected stdout (exactly):
    //   final acc=3

    let (stdout, _warnings) = run_sim(sv, "tb", "enclosing").expect("simulation should run");
    assert_eq!(stdout, "final acc=3\n");
}

/// (b) The classic early-exit idiom: a loop wrapped in a named block that
/// disables ITSELF (`begin : search … if (cond) disable search; end`).  The
/// statements after the disabled block still run.
#[test]
fn disable_self_named_loop_block() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    integer i;
    reg [7:0] mem [0:3];

    initial begin
        mem[0] = 8'd10;
        mem[1] = 8'd20;
        mem[2] = 8'd0;
        mem[3] = 8'd30;
        begin : search
            for (i = 0; i < 4; i = i + 1) begin
                if (mem[i] == 0) disable search;
                $display("check %0d value %0d", i, mem[i]);
            end
        end
        $display("first zero at index %0d", i);
        $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  i=0: mem[0]=10 != 0 -> print "check 0 value 10"; i=1: 20 != 0 ->
    //        print "check 1 value 20"; i=2: mem[2]==0 -> `disable search`
    //        exits the named block before this iteration's display runs.
    //        Execution resumes AFTER the block with i still 2.
    //
    // Expected stdout (exactly):
    //   check 0 value 10
    //   check 1 value 20
    //   first zero at index 2

    let (stdout, _warnings) = run_sim(sv, "tb", "selfnamed").expect("simulation should run");
    assert_eq!(
        stdout,
        "check 0 value 10\ncheck 1 value 20\nfirst zero at index 2\n"
    );
}

/// (c) `disable <taskname>;` inside that task = an early return: the output
/// formal keeps the value assigned before the disable and later statements
/// of the task do not run.  This variant uses a delay-free task (compiled as
/// a standalone C function, so the disable lowers to an early C return).
#[test]
fn disable_task_inside_task_plain_early_return() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [3:0] r;

    task set_if_nonzero(input [3:0] v);
        begin
            if (v == 4'd0) disable set_if_nonzero;
            r = v;
            $display("assigned %0d", v);
        end
    endtask

    initial begin
        set_if_nonzero(4'd5);
        set_if_nonzero(4'd0);
        $display("r=%0d", r);
        $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  call(5): v != 0 -> r=5, print "assigned 5".
    //        call(0): v == 0 -> `disable set_if_nonzero` returns from the
    //        task immediately: r keeps 5, no display for this call.
    //        Then print r.
    //
    // Expected stdout (exactly):
    //   assigned 5
    //   r=5

    let (stdout, _warnings) = run_sim(sv, "tb", "taskret").expect("simulation should run");
    assert_eq!(stdout, "assigned 5\nr=5\n");
}

/// (c') Same early-return semantics inside a WAIT-BEARING task, which is
/// INLINED at its call sites: the disable must jump to that expansion's done
/// label (not emit a C return from the caller's coroutine), skipping the
/// remaining delay + display of the second call only.
#[test]
fn disable_inlined_task_early_return() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg [3:0] r;

    task wait_set(input [3:0] v);
        begin
            if (v == 4'd0) begin
                $display("skip zero");
                disable wait_set;
            end
            #2 r = v;
            $display("wait assigned %0d", v);
        end
    endtask

    initial begin
        r = 4'd0;
        wait_set(4'd3);
        wait_set(4'd0);
        $display("r=%0d t=%0t", r, $time);
        $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  call(3): inline expansion: v != 0 -> suspend #2.
    //   t=2  r=3, print "wait assigned 3".  Second call's expansion: v == 0
    //        -> print "skip zero" then `disable wait_set` jumps to that
    //        expansion's done label, skipping ITS #2 and display.  No time
    //        passes for the skipped delay.
    //        Then print r=3 at t=2.
    //
    // Expected stdout (exactly):
    //   wait assigned 3
    //   skip zero
    //   r=3 t=2

    let (stdout, _warnings) = run_sim(sv, "tb", "inlret").expect("simulation should run");
    assert_eq!(stdout, "wait assigned 3\nskip zero\nr=3 t=2\n");
}

/// (d) break/continue inside every loop shape, iteration counts pinned.
#[test]
fn break_continue_all_loop_shapes() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    integer i;
    reg [7:0] b, c;

    initial begin
        b = 0; c = 0;
        // continue skips the body but STILL runs the increment; break skips
        // both.  b increments for i in {0,1,2,4,5,6}; at i==7 break fires.
        for (i = 0; i < 10; i = i + 1) begin
            if (i == 3) continue;
            if (i == 7) break;
            b = b + 1;
        end
        $display("for: b=%0d i=%0d", b, i);

        // while: i increments first each pass.  c counts {1,2,3,5,6,7};
        // i==4 continues, i==8 breaks.
        i = 0; c = 0;
        while (i < 10) begin
            i = i + 1;
            if (i == 4) continue;
            if (i == 8) break;
            c = c + 1;
        end
        $display("while: c=%0d i=%0d", c, i);

        // repeat: five of six iterations execute; k==4 continues (nothing
        // after it anyway), k==5 breaks out of iteration five.
        c = 0;
        repeat (6) begin
            c = c + 1;
            if (c == 4) continue;
            if (c == 5) break;
        end
        $display("repeat: c=%0d", c);

        // forever: pure counter with a break exit.
        i = 0;
        forever begin
            i = i + 1;
            if (i >= 5) break;
        end
        $display("forever: i=%0d", i);
        $finish;
    end
endmodule
"#;

    // Hand-simulation (all at t=0):
    //   for:     i=0,1,2 increment b (b=3); i=3 continue; i=4,5,6 increment
    //            (b=6); i=7 break (increment skipped) -> b=6, i stays 7.
    //   while:   i=1..3 count c (c=3); i=4 continue; i=5..7 count (c=6);
    //            i=8 break -> c=6, i=8.
    //   repeat:  iterations produce c=1,2,3 (no jump); c=4 continue ends
    //            iteration four; c=5 break during iteration five -> c=5.
    //   forever: i reaches 5 -> break.
    //
    // Expected stdout (exactly):
    //   for: b=6 i=7
    //   while: c=6 i=8
    //   repeat: c=5
    //   forever: i=5

    let (stdout, _warnings) = run_sim(sv, "tb", "jumpshapes").expect("simulation should run");
    assert_eq!(
        stdout,
        "for: b=6 i=7\nwhile: c=6 i=8\nrepeat: c=5\nforever: i=5\n"
    );
}

/// (e) Nesting: `break` exits the INNERMOST loop only, while `disable` of a
/// named level exits that level regardless of how deep the disable sits.
#[test]
fn nested_break_vs_disable_levels() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    integer i, j, hits;
    reg [7:0] out;

    initial begin
        hits = 0;
        out = 0;
        begin : outer_blk
            for (i = 0; i < 3; i = i + 1) begin : mid_blk
                for (j = 0; j < 3; j = j + 1) begin
                    if (i == 1 && j == 1) break;          // innermost only
                    if (i == 2 && j == 0) disable outer_blk;
                    hits = hits + 1;
                end
                out = out + 16;                            // per completed inner loop
            end
        end
        $display("hits=%0d out=%0d", hits, out);
        $finish;
    end
endmodule
"#;

    // Hand-simulation (all at t=0):
    //   i=0: j=0 hits=1; j=1 hits=2 (guards need i>=1); j=2 hits=3. Inner
    //        completes -> out=16.
    //   i=1: j=0 hits=4 (`i==1 && j==1` needs j==1 too). j=1 -> break exits
    //        the INNER loop only; out=32; the mid loop continues with i=2.
    //   i=2: j=0 `disable outer_blk` -> jumps past the WHOLE named block,
    //        skipping the mid-loop's out update and all further iterations.
    //        Display runs (it sits outside the disabled block): hits=4,
    //        out=32.
    //
    // Expected stdout (exactly):
    //   hits=4 out=32

    let (stdout, _warnings) = run_sim(sv, "tb", "nested").expect("simulation should run");
    assert_eq!(stdout, "hits=4 out=32\n");
}

/// (f) Cross-process disable is rejected cleanly at codegen time naming the
/// unsupported case (never mis-lowered into a same-function goto).  The
/// target block belongs to ANOTHER initial, so it cannot be matched against
/// the disabling process's enclosing scopes and codegen fails before any C
/// is emitted.
#[test]
fn cross_process_disable_rejected() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    reg done;
    initial begin : victim
        done = 1'b1;
    end
    initial begin
        disable victim;   // targets ANOTHER initial's block: not supported
        $finish;
    end
endmodule
"#;

    let err = codegen_error(sv, "tb", "crossproc").expect("codegen should fail");
    assert!(
        err.contains("disable") && err.contains("not supported"),
        "unexpected error message: {err}"
    );
}

/// Disabling a named fork would require terminating a separate child
/// coroutine, so the v1 codegen contract rejects it instead of lowering it as
/// a same-process block jump.
#[test]
fn named_fork_disable_rejected() {
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    initial begin
        fork : workers
            begin
                #5 $display("WRONG: worker ran");
            end
        join_none
        disable workers;
        $display("after disable");
        $finish;
    end
endmodule
"#;

    let err = codegen_error(sv, "tb", "namedfork").expect("codegen should fail");
    assert!(
        err.contains("disable of `workers`") && err.contains("named forks"),
        "unexpected error message: {err}"
    );
}

/// (b') Disabling a named block that IS the loop body terminates only that
/// execution of the block (LRM §11): the increment and condition still run,
/// so the loop continues with the next iteration — the Verilog-1995
/// emulation of `continue`.  (Early EXIT requires the wrapping shape of
/// case (b), where the named block contains the whole loop.)
#[test]
fn disable_loop_body_block_is_iteration_scoped() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    integer i, count;

    initial begin
        count = 0;
        for (i = 0; i < 6; i = i + 1) begin : body
            count = count + 1;
            if (i == 2) disable body;
            $display("iter %0d", i);
        end
        $display("count=%0d", count);
        $finish;
    end
endmodule
"#;

    // Hand-simulation (all at t=0):
    //   i=0: count=1, print "iter 0".
    //   i=1: count=2, print "iter 1".
    //   i=2: count=3, `disable body` ends THIS block execution -> "iter 2"
    //        skipped, but the increment still runs.
    //   i=3,4,5: count=4,5,6, each prints.
    //
    // Expected stdout (exactly):
    //   iter 0
    //   iter 1
    //   iter 3
    //   iter 4
    //   iter 5
    //   count=6

    let (stdout, _warnings) = run_sim(sv, "tb", "bodyblock").expect("simulation should run");
    assert_eq!(stdout, "iter 0\niter 1\niter 3\niter 4\niter 5\ncount=6\n");
}

/// (b'') Same iteration-scoped self-disable as (b'), but the `disable`
/// line carries a UTF-8 comment BEFORE the keyword (`é`, `µ`, `—` are
/// multi-byte).  The source-line recovery of the target identifier scans
/// that line BYTE-wise: a character-based slice at a byte offset landing
/// inside a multi-byte codepoint used to panic during Db::build (taking
/// down LSP/llg).  The design must compile and run correctly.
#[test]
fn disable_line_with_utf8_comment_recovers_target() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module tb;
    integer i, count;

    initial begin
        count = 0;
        for (i = 0; i < 6; i = i + 1) begin : body
            count = count + 1;
            /* durée µs — note */ if (i == 2) begin disable body; end
            $display("iter %0d", i);
        end
        $display("count=%0d", count);
        $finish;
    end
endmodule
"#;

    // Hand-simulation: identical to (b') — the nested-construct disable
    // target is recovered from this exact source line,
    // whose leading multi-byte characters must not disturb the byte scan.
    //
    // Expected stdout (exactly):
    //   iter 0
    //   iter 1
    //   iter 3
    //   iter 4
    //   iter 5
    //   count=6

    let (stdout, _warnings) = run_sim(sv, "tb", "utf8dis").expect("simulation should run");
    assert_eq!(stdout, "iter 0\niter 1\niter 3\niter 4\niter 5\ncount=6\n");
}

/// (g) Optimization parity: a design mixing break, continue and disable must
/// produce identical stdout with every pass on vs off.
#[test]
fn opt_parity_break_continue_disable() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let dir = sim_harness::TempDir::new("disable-parity").expect("create temp dir");
    let src = dir.path().join("tb.sv");
    let sv = r#"module tb;
    integer i, k, sum;
    reg clk;

    initial begin : main
        sum = 0;
        // Wrapping named block + self-disable: the classic EARLY EXIT.
        // NOTE: the disable guard must not be shadowed by the continue
        // guard — 9 % 3 == 0 would make the continue fire first.
        begin : outer
            for (i = 0; i < 12; i = i + 1) begin
                if (i % 3 == 0) continue;
                if (i == 8) disable outer;
                sum = sum + i;
            end
        end
        $display("sum=%0d i=%0d", sum, i);
        k = 0;
        repeat (10) begin
            k = k + 1;
            if (k == 3) continue;
            if (k == 6) break;
            sum = sum + k;
        end
        $display("sum=%0d k=%0d", sum, k);
    end

    always #5 clk = ~clk;
    initial #40 $finish;
endmodule
"#;
    std::fs::write(&src, sv).expect("write source");

    let _guard = CWD_LOCK.lock().unwrap();
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
        let db = llg::core::db::Db::from_slang(&out.snapshot).map_err(|e| format!("db: {e}"))?;
        let on = sim::codegen::generate_from_db_with_opts(&db, &OptConfig::default())
            .map_err(|e| format!("codegen(opt-on): {e}"))?;
        let off = sim::codegen::generate_from_db_with_opts(&db, &OptConfig::none())
            .map_err(|e| format!("codegen(opt-off): {e}"))?;
        let on_out = run_variant(dir.path(), "opt_on", &on.model_c)?;
        let off_out = run_variant(dir.path(), "opt_off", &off.model_c)?;
        Ok((on_out, off_out))
    });

    let (on, off) = result.expect("both variants should run");

    // Reference trace (hand-computed):
    //   for: multiples of 3 continue (no accumulate); at i==8 `disable
    //   outer` exits the WHOLE wrapping block -> sum = 1+2+4+5+7 = 19,
    //   i stays 8.
    //   repeat: k=1,2 add (sum=22); k=3 continue; k=4,5 add (sum=31); k=6
    //   breaks -> sum=31, k=6.
    assert_eq!(on, "sum=19 i=8\nsum=31 k=6\n");
    assert_eq!(off, on, "opt-on and opt-off runs diverged");
}
