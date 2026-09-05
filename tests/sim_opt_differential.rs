//! Optimization on/off differential harness.
//!
//! For each design: one Surelog compile and one owned-DB build, then
//! `codegen::generate_from_db_with_opts` twice — once with
//! [`sim::opt::OptConfig::default`] (all passes) and once
//! with `OptConfig::none` — building BOTH models with the CMake builder
//! (`sim::build::build_model_cmake`) and running both executables.  The two
//! stdouts must be byte-identical: every optimization pass must be
//! semantics-preserving.  (Correctness against hand-simulated expectations is
//! already pinned by the per-feature suites, which run the default
//! configuration.)

#[path = "support/sim.rs"]
mod sim_harness;
use std::sync::Mutex;

use llg::core::compile;
use llg::sim;
use llg::sim::opt::OptConfig;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Compile `sv` once (top `top`), generate + build + run both variants in
/// PID-keyed sibling directories, and return their two stdouts.
fn run_both(sv: &str, top: &str, tag: &str) -> Result<(String, String), String> {
    sim_harness::with_temp_cwd(tag, |dir| {
        let src = dir.join("tb.sv");
        std::fs::write(&src, sv).map_err(|error| format!("write source: {error}"))?;
        // 1. Surelog compile + elaborate (once).
        let out = compile::compile_checked(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some(top.to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        let database = llg::core::db::Db::build(design).map_err(|error| error.to_string())?;

        // 2. Both configurations from one owned DB. Besides avoiding a second
        // VPI walk, this keeps differential generation independent of
        // consumable frontend iterator relationships.
        let opt_on = sim::codegen::generate_from_db_with_opts(&database, &OptConfig::default())
            .map_err(|e| format!("codegen(opt-on): {e}"))?;
        let opt_off = sim::codegen::generate_from_db_with_opts(&database, &OptConfig::none())
            .map_err(|e| format!("codegen(opt-off): {e}"))?;

        // 3. Build both models into separate directories (CMake).
        let run_variant = |name: &str, model_c: &str| -> Result<String, String> {
            let out_dir = dir.join(name);
            let exe = sim::build::build_model_cmake(&out_dir, &[("model.c", model_c)])
                .map_err(|e| format!("cmake({name}): {e}"))?;
            sim_harness::run_executable(&exe).map_err(|error| format!("{name}: {error}"))
        };
        let on = run_variant("opt_on", &opt_on.model_c)?;
        let off = run_variant("opt_off", &opt_off.model_c)?;
        Ok((on, off))
    })
}

fn assert_differential(sv: &str, top: &str, tag: &str) {
    // Hold the lock only while compiling/building/running (cwd safety);
    // release it BEFORE asserting, so a failure here cannot poison
    // SURELOG_LOCK and cascade into every later test in this binary.
    let both = {
        let _guard = SURELOG_LOCK.lock().unwrap();
        run_both(sv, top, tag)
    };
    let (on, off) = both.expect("both variants should run");
    assert_eq!(on, off, "opt-on and opt-off runs diverged ({tag})");
}

/// Event-controlled always blocks (edge or-lists) plus scaled `#` delays and
/// NBA accumulation — exercises WaitEvents sources, SensLoop drivers and the
/// delay/`$time` paths under folding/pruning/storage pruning.
#[test]
fn diff_event_always_and_delays() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module counter #(parameter WIDTH = 8) (
        input logic clk, input logic rst_n,
        output logic [WIDTH-1:0] count);
    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) count <= '0;
        else count <= count + 1;
    end
endmodule

module tb;
    reg clk; reg rst_n; wire [7:0] count;
    counter #(.WIDTH(8)) u(.clk(clk), .rst_n(rst_n), .count(count));
    always #5 clk = ~clk;
    initial begin
        clk = 0; rst_n = 1;
        #1 rst_n = 0;
        #3 rst_n = 1;
        #40 $display("count=%0d t=%0t", count, $time);
        $finish;
    end
endmodule
"#;
    assert_differential(sv, "tb", "events");
}

/// casez with constant selectors: pruning must pick exactly the arm the
/// unpruned if/else chain picks (wildcard match, fall-through to default,
/// first-match-wins).
#[test]
fn diff_casez_matching() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [3:0] sel;
    initial begin
        sel = 4'b1000;
        casez (sel)
            4'b1z0z: $display("first hit sel=%b", sel);
            4'b0101: $display("no");
            default: $display("default");
        endcase
        sel = 4'b0101;
        casez (sel)
            4'b1z0z: $display("no");
            4'b01z1: $display("second hit");
            default: $display("default");
        endcase
        sel = 4'b0011;
        casez (sel)
            4'b1z0z: $display("no");
            4'b01z1: $display("no");
            default: $display("third default");
        endcase
        $finish;
    end
endmodule
"#;
    assert_differential(sv, "tb", "casez");
}

/// Function calls in expressions and task calls with output/inout formals —
/// exercises the Call node's argument expressions under both configurations.
#[test]
fn diff_function_and_task_calls() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] data;
    reg [7:0] out;
    reg [3:0] sum;

    function [7:0] mix(input [7:0] a, input [7:0] b);
        mix = a ^ b;
    endfunction

    // NOTE: a select LHS on a formal (`o[i] = v;`) is outside the v1
    // lowering subset ("cannot resolve base signal of select"); tasks assign
    // whole formals here.
    task poke(inout [7:0] o, input [7:0] v);
        o = v;
    endtask

    task add_one(input [3:0] x, output [3:0] y);
        y = x + 4'd1;
    endtask

    initial begin
        data = 8'h0f;
        out = mix(data, 8'hf0);
        $display("mix=%h", out);
        poke(out, data ^ 8'h3c);
        $display("poked=%h", out);
        add_one(data[3:0], sum);
        $display("sum=%b", sum);
        #5 data = data + 8'h10;
        $display("data=%h t=%0t", data, $time);
        $finish;
    end
endmodule
"#;
    assert_differential(sv, "tb", "funcs");
}

/// fork/join with branch coroutines writing signals and displaying —
/// exercises pre_fn Branch bodies (their reads/writes/waits are collected
/// like any other statement position).
#[test]
fn diff_fork_join() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
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
    assert_differential(sv, "tb", "fork");
}

/// `wait (cond)` level-sensitive blocking with a fork partner — exercises
/// WaitCond spin loops, sensitivity sets and constant-condition handling.
#[test]
fn diff_wait_cond() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg ready;
    reg [7:0] data;
    initial begin
        ready = 0;
        data = 8'h00;
        fork
            begin #20 ready = 1; end
            begin
                wait (ready == 1'b1) $display("waiter saw ready at t=%0t", $time);
            end
        join
        data = 8'ha5;
        wait (!ready) $display("never: %h", data);
        $display("after waits data=%h t=%0t", data, $time);
        $finish;
    end
endmodule
"#;
    assert_differential(sv, "tb", "waitcond");
}

/// A signal referenced ONLY as a wait-event source (`b`: never read by an
/// expression, never written) must keep its storage under `unused_storage`
/// pruning — the emitted `llg_event_spec_t` array names its global.
#[test]
fn diff_wait_event_only_source() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg a;
    reg b;
    initial begin
        a = 1'b1;
        @(posedge b);
        $display("fired a=%b", a);
    end
    initial begin
        #10 $display("stop a=%b", a);
        $finish;
    end
endmodule
"#;
    assert_differential(sv, "tb", "evtsource");
}

/// `$monitor` + `$strobe`: clocked toggles and an NBA counter printed by
/// `$monitor`, plus a `$strobe` in an independent initial landing on a
/// posedge step (so it prints post-NBA values) — exercises MonEval argument
/// collection end-to-end against the unused-storage collector (`mon_only` is
/// referenced ONLY as a monitor argument, so losing that collection breaks
/// the opt-on build).  Separate initials keep each timeline self-evident;
/// `$finish` stops the run at t=32.
#[test]
fn diff_monitor_and_strobe() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg clk;
    reg [3:0] cnt;
    reg mon_only;
    always #5 clk = ~clk;
    always @(posedge clk) cnt <= cnt + 4'd1;
    initial begin
        clk = 1'b0;
        cnt = 4'd0;
        $monitor("mon t=%0t clk=%b cnt=%0d mo=%b", $time, clk, cnt, mon_only);
    end
    initial begin
        #25 $strobe("strobe t=%0t cnt=%0d", $time, cnt);
    end
    initial begin
        #32 $finish;
    end
endmodule
"#;
    assert_differential(sv, "tb", "monitor");
}

/// A task copy-out target that is otherwise unreferenced: `sink`'s sole
/// mention is the `fill` call (output formal) plus ONE display after the call
/// returns.  Dropping or corrupting the copy-out flips that display (prints
/// x instead of the computed byte) rather than just failing compilation, so
/// a collector regression here surfaces as a stdout divergence.
#[test]
fn diff_task_copyout_sole_reference() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] seed;
    reg [7:0] sink;
    task fill(input [7:0] v, output [7:0] o);
        o = v ^ 8'h5a;
    endtask
    initial begin
        seed = 8'h3c;
        fill(seed, sink);
        $display("sink=%h", sink);
        $finish;
    end
endmodule
"#;
    assert_differential(sv, "tb", "copyout");
}

/// break/continue + disable control flow: every loop shape carries
/// goto-label pairs after lowering, and the named-block disable adds a jump
/// across the loop boundary — folding/pruning must never reorder or drop
/// statements across those Goto boundaries (opt-on must print exactly what
/// opt-off prints).
#[test]
fn diff_break_continue_disable() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    integer i, k, sum;
    reg clk;

    initial begin : main
        sum = 0;
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
    assert_differential(sv, "tb", "jumpctrl");
}

/// Combinational (`SensLoop`) process whose body carries CONTROL FLOW:
/// a `for` with `continue` + `break` and a self-disable of its own named
/// block.  Such bodies render TWICE in emit_c (first evaluation + the
/// re-evaluation copy inside `wait_any`), and every `_bk`/`_ct`/`_xb`
/// label is defined once per copy — the `_r` relabel path must keep each
/// copy's gotos internal or the generated model.c fails to compile with
/// duplicate-label errors (covered implicitly by a successful build here)
/// or jumps across copies (surfacing as a stdout divergence).
///
/// NOTE: this must be a BARE `always_comb`.  An explicit `@*` reaches
/// lowering as an event control whose synthesized `WaitAny` statement makes
/// the process `Loop`-shaped (single render, no labels duplicated).
#[test]
fn diff_comb_body_control_flow_sensloop_relabel() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    integer k;
    reg [3:0] din;
    reg [7:0] acc;
    reg done;

    always_comb begin : comb_blk
        acc = 8'd0;
        done = 1'b0;
        for (k = 0; k < 8; k = k + 1) begin
            if (k == 2) continue;
            if (k == 6) break;
            if (din == 4'hf) disable comb_blk;
            acc = acc + din;
        end
        done = 1'b1;
    end

    initial begin
        din = 4'd1;
        #1 $display("acc=%0d done=%b", acc, done);
        din = 4'd2;
        #1 $display("acc=%0d done=%b", acc, done);
        din = 4'hf;
        #1 $display("acc=%0d done=%b", acc, done);
        $finish;
    end
endmodule
"#;

    // Hand-computed trace (the comb process re-evaluates after every din
    // change; k==2 continues, k==6 breaks, so k ∈ {0,1,3,4,5} accumulate):
    //   t=0 settle din=1 -> acc = 5*1 = 5, done=1
    //   t=1 print, then din=2 -> acc = 10, done=1
    //   t=2 print, then din=f -> acc=0, done=0, then k=0 sees din==f and
    //      `disable comb_blk` exits the WHOLE named block (done stays 0)
    //   t=3 print
    let expected = "acc=5 done=1\nacc=10 done=1\nacc=0 done=0\n";
    let both = {
        let _guard = SURELOG_LOCK.lock().unwrap();
        run_both(sv, "tb", "combctl")
    };
    let (on, off) = both.expect("both variants should run");
    assert_eq!(
        on, expected,
        "opt-on run diverged from the hand-computed trace"
    );
    assert_eq!(off, on, "opt-on and opt-off runs diverged");
}

/// Procedural continuous assignment (`assign x = src;` / `deassign x;`):
/// every PCA site lowers to a dedicated enable-guarded process plus plain
/// blocking writes from the statements — the optimizer must keep the enable
/// signal (read by the guard's `If` condition and wait list, written by the
/// statements) and must never prune the non-constant guard, or the opt-on
/// run diverges.
#[test]
fn diff_procedural_continuous_assign() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] x;
    reg [7:0] src;
    reg en;

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
        #2 force x = 8'hff;
        #2 src = 8'h44;
        #2 $display("forced x=%h t=%0t", x, $time);
        release x;
        #2 src = 8'h55;
        #2 $display("released-then-driven x=%h t=%0t", x, $time);
        $finish;
    end
endmodule
"#;

    // Hand-computed trace (same model as sim_force::sim_pca_lifecycle plus a
    // force/release interleave):
    //   t=4   x=22 (assigned)
    //   t=6   x=22 (deassigned: held)
    //   t=8   x=33 (re-assigned)
    //   t=10  force wins (saved value: 33); the PCA write after src=44 at
    //         t=12 is DROPPED while forced
    //   t=14  forced x=ff displayed; release restores the saved 33
    //   t=16  src=55 re-drives x=55 through the still-enabled site
    //   t=18  released-then-driven x=55 displayed
    let expected = "x=22 t=4\nx=22 t=6\nx=33 t=8\nforced x=ff t=14\n\
                    released-then-driven x=55 t=18\n";
    let both = {
        let _guard = SURELOG_LOCK.lock().unwrap();
        run_both(sv, "tb", "pca")
    };
    let (on, off) = both.expect("both variants should run");
    assert_eq!(
        on, expected,
        "opt-on run diverged from the hand-computed trace"
    );
    assert_eq!(off, on, "opt-on and opt-off runs diverged");
}

/// Structural gate network: n-input gates, a delayed `not`, an enable gate,
/// pullup/pulldown constant drivers, and an always_ff sampling gate outputs.
/// The gate processes are composed from the same IR shapes as continuous
/// assignments (SensLoop + whole-signal writes), but this pins that the
/// optimizer keeps every gate-driven signal (written) and every gate-input
/// signal (read + sensitivity), and that folding/identity passes do not
/// disturb the sv4 op chains.
#[test]
fn diff_gate_network() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    // Hand-computed trace (spawn order: gate combs, then always, then
    // initials; the delayed `not` registers its t=2 timer at t=0):
    //   t=0 initial: display -> t=x y=x e=x pu=1 pdd=0
    //   t=1 a=b=1            -> t=1 (and wakes); x = xnor(b,b) = 0
    //   t=2 delayed not fires -> y=~1=0; bufif1(en=1) -> e=0
    //   t=4 display          -> t=1 y=0 x=0 e=0
    //   t=4 en=0             -> e=z (bufif1 disabled)
    //   t=6 display          -> z
    //   t=8 display          -> t=1 y=0 e=z; clk edges fill hist (never read)
    let sv = r#"`timescale 1ns/1ps
module tb;
    reg a, b, en, clk;
    wire t, y, x, e;
    wire pu, pdd;
    reg [2:0] hist;
    and  ga(t, a, b);
    xnor gx(x, b, b);
    not #2 gb(y, t);
    bufif1 ge(e, y, en);
    pullup pp(pu);
    pulldown gp(pdd);
    always @(posedge clk) hist <= {e, pu, pdd};
    initial begin
        clk = 1'b0;
        en = 1'b1;
        $display("t=%0t %b_%b_%b_%b_%b", $time, t, y, e, pu, pdd);
        #1 a = 1'b1; b = 1'b1;
        #3 $display("t=%0t %b_%b_%b_%b", $time, t, y, x, e);
        en = 1'b0;
        #2 $display("t=%0t %b", $time, e);
        #2 $display("t=%0t %b_%b_%b", $time, t, y, e);
        #5 $finish;
    end
    always #5 clk = ~clk;
endmodule
"#;
    assert_differential(sv, "tb", "gates");
}

/// Dynamic true-net declaration assignments share the ordinary continuous
/// assignment IR shape. Optimization must preserve both their precomputed
/// sensitivity reads and four-state/vector assignment context.
#[test]
fn diff_dynamic_true_net_declaration_assignment() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"// llg-test-fixture: tests/sim_opt_differential.rs/net_decl.sv
module tb;
    logic [7:0] a = 8'h7f;
    logic [7:0] b = 8'h01;
    wire [8:0] sum = a + b;
    wire [15:0] bits = {a, b};
    initial begin
        $display("%h %b", sum, bits);
        a = 8'hff;
        #1 $display("%h %b", sum, bits);
        b = 8'b10xz0011;
        #1 $display("%h %b", sum, bits);
        $finish;
    end
endmodule
"#;
    assert_differential(sv, "tb", "net_decl");
}
