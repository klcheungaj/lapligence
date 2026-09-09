//! End-to-end simulator tests for delayed assignments: intra-assignment
//! delays (`a = #5 b;`, `a <= #5 b;` — LRM 1364-1995 §9.7.4) and
//! continuous-assignment delays (`assign #2 y = a;` — §1364-1995 §6.1.3),
//! procedural parameter/constant-expression delays, plus clean codegen
//! rejections for event/repeat/dynamic forms. Slang compile → codegen →
//! CMake build → run, asserting exact
//! stdout against hand-simulated traces.
//!
//! Tests run with the CWD pointed at a fresh temp dir and serialize process-CWD
//! changes with the other native integration tests.
#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::compile;
use llg::sim;

fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    sim_harness::run_sim(sv, "tb", tag)
}

/// Compile + codegen `sv`, returning the raw codegen result (for rejection
/// message assertions).
fn codegen_result(
    sv: &str,
    tag: &str,
) -> Result<Result<sim::codegen::GeneratedModel, String>, String> {
    sim_harness::with_frontend_temp_cwd(tag, |dir| {
        let src = dir.join("tb.sv");
        std::fs::write(&src, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile_checked(&compile::CompileOpts {
            files: vec![src.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        let db =
            llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
        Ok(sim::codegen::generate(&db).map_err(|error| error.to_string()))
    })
}

/// (a) Blocking intra-assignment delay: the RHS is evaluated at execution
/// time, not when the LHS is updated — `b=1; a=#5 b; b=2;` must leave `a`
/// holding 1 even though `b` is 2 by the time the assignment lands at t=5.
#[test]
fn sim_intra_delay_blocking_ordering() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] a, b;

    initial begin
        b = 8'd1;
        a = #5 b;
        b = 8'd2;
        $display("t=%0t a=%0d b=%0d", $time, a, b);
    end
endmodule
"#;

    // Hand-simulation (LRM 1364-1995 §9.7.4):
    //   t=0  b=1; `a = #5 b` evaluates the RHS (b==1) into a temp and
    //        SUSPENDS the process until t=5.
    //   t=5  a := 1 (the captured value); the process continues: b=2;
    //        $display shows a=1 (early RHS), b=2.
    //
    // Expected stdout (exactly):
    //   t=5 a=1 b=2
    let stdout = run_sim(sv, "blk").expect("simulation should run");
    assert_eq!(stdout, "t=5 a=1 b=2\n", "stdout: {stdout}");
}

/// (b) NBA intra-assignment delay commit time: `a <= #4 8'h07;` executed at
/// t=0 lands in the NBA region of t=4.  A plain read at t=2 sees nothing,
/// `$strobe` (end-of-step values) pins the commit inside step t=4, and the
/// writer itself observes the committed value afterwards. The current backend's
/// executing process suspends across the delay window for both assignment
/// kinds — see the documented approximation in src/sim/AGENTS.md.)
#[test]
fn sim_intra_delay_nba_commit_time() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] a;

    initial begin
        #2 $display("t=%0t a=%h", $time, a);
        #2 $strobe("strobe t=%0t a=%h", $time, a);
    end

    initial begin
        a = 8'h00;
        a <= #4 8'h07;
        #1 $display("t=%0t a=%h", $time, a);
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  writer: a=00 (X->0); `a <= #4 07` evaluates the RHS into a
    //        temp and suspends until t=4.
    //   t=2  prober display a=00 (nothing committed yet).
    //   t=4  ACTIVE region: the writer resumes and RECORDS the NBA (temp
    //        value 07); the NBA region commits a := 07 within the same
    //        step.  $strobe runs with the post-step values: a=07.
    //   t=5  writer's own read after the commit: a=07.
    //
    // $strobe defers to the end of the step, so every assertion holds
    // regardless of the two processes' wakeup order at t=4.
    //
    // Expected stdout (exactly):
    //   t=2 a=00
    //   strobe t=4 a=07
    //   t=5 a=07
    let stdout = run_sim(sv, "nba").expect("simulation should run");
    assert_eq!(
        stdout, "t=2 a=00\nstrobe t=4 a=07\nt=5 a=07\n",
        "stdout: {stdout}"
    );
}

/// (c) Continuous-assignment delay: the output lags every input change by
/// exactly D (input changes spaced further apart than D, where the
/// no-pulse-filter approximation agrees with the LRM).
#[test]
fn sim_ca_delay_lags_by_d() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg src;
    wire [3:0] y;

    assign #2 y = {3'b000, src};

    initial begin
        $display("t=%0t y=%b", $time, y);
        src = 1'b1;
        #3 src = 1'b0;
        #1 $display("t=%0t y=%b", $time, y);
        #2 $display("t=%0t y=%b", $time, y);
    end
endmodule
"#;

    // Hand-simulation (body = delay D, then write CURRENT rhs):
    //   t=0  CA starts its first evaluation but suspends for #2 (nothing
    //        written yet).  Display y=xxxx.  src X->1.
    //   t=2  CA writes y = current src = 0001, then waits on src.
    //   t=3  src 1->0 wakes the CA, which suspends for #2 (until t=5).
    //   t=4  display y=0001 (change has not landed yet).
    //   t=5  CA writes y = current src = 0000.
    //   t=6  display y=0000.
    //
    // Expected stdout (exactly):
    //   t=0 y=xxxx
    //   t=4 y=0001
    //   t=6 y=0000
    let stdout = run_sim(sv, "calag").expect("simulation should run");
    assert_eq!(
        stdout, "t=0 y=xxxx\nt=4 y=0001\nt=6 y=0000\n",
        "stdout: {stdout}"
    );
}

/// (c) A parameter-valued CA delay (`assign #P …`) folds through the
/// collected parameter values and scales like a literal delay.
#[test]
fn sim_ca_delay_parameter_scales() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    parameter P = 2;
    reg src;
    wire y;

    assign #P y = src;

    initial begin
        src = 1'b1;
        #1 $display("t=%0t y=%b", $time, y);
        #2 $display("t=%0t y=%b", $time, y);
    end
endmodule
"#;

    // Hand-simulation (P=2, 1ns/1ps default timescale):
    //   t=0  src=1; CA suspends for its first-evaluation #P.
    //   t=1  display y=x (write due at t=P).
    //   t=2  CA writes y=1.
    //   t=3  display y=1.
    //
    // Expected stdout (exactly):
    //   t=1 y=x
    //   t=3 y=1
    let stdout = run_sim(sv, "capar").expect("simulation should run");
    assert_eq!(stdout, "t=1 y=x\nt=3 y=1\n", "stdout: {stdout}");
}

/// (d) The continuous assign's t=0 first evaluation ALSO waits D: with
/// nothing else driving `src`, `y` stays X until t=D even though `src` was
/// set to 1 in the same active region.
#[test]
fn sim_ca_delay_t0_evaluation_delayed() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg src;
    wire y;

    assign #3 y = src;

    initial begin
        src = 1'b1;
        $display("t=%0t y=%b", $time, y);
        #2 $display("t=%0t y=%b", $time, y);
        #1 $display("t=%0t y=%b", $time, y);
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  CA suspends for its first-evaluation #3.  src=1.  Display
    //        y=x (an UNDELAYED assign would already print 1 here).
    //   t=2  still x (write due at t=3).
    //   t=3  CA writes y=current src=1; display y=1.
    //
    // Expected stdout (exactly):
    //   t=0 y=x
    //   t=2 y=x
    //   t=3 y=1
    let stdout = run_sim(sv, "cat0").expect("simulation should run");
    assert_eq!(stdout, "t=0 y=x\nt=2 y=x\nt=3 y=1\n", "stdout: {stdout}");
}

/// A delayed continuous assignment rejects a short pulse and schedules the
/// later stable transition using inertial delay semantics.
#[test]
#[ignore = "DELAY-BUG: delayed continuous assignments need inertial update scheduling"]
fn sim_ca_delay_rejects_short_pulse() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg src;
    wire y;

    assign #3 y = src;

    initial begin
        src = 1'b0;
        #1 src = 1'b1;
        #1 src = 1'b0;
        #1 $display("t=%0t y=%b", $time, y);
        #1 src = 1'b1;
        #2 $display("t=%0t y=%b", $time, y);
        #1 $display("t=%0t y=%b", $time, y);
        $finish;
    end
endmodule
"#;

    // IEEE inertial scheduling rejects the short pulse, leaving y unknown
    // until the stable transition scheduled for t=7.
    let stdout = run_sim(sv, "current").expect("simulation should run");
    assert_eq!(stdout, "t=3 y=x\nt=6 y=x\nt=7 y=1\n");
}

/// (e) Timescale interplay: in a module with `` `timescale 10ns/1ns `` the
/// CA's `#2` scales exactly like a plain `#2` statement (20 scheduler ticks
/// of the 1ns design precision), so `y` flips between two plain `#1` steps
/// and stays stable across a plain `#2` window.
#[test]
fn sim_ca_delay_timescale_scaling() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 10ns/1ns
module tb;
    reg src;
    wire y;

    assign #2 y = src;

    initial begin
        src = 1'b1;
        #1 $display("t=%0t y=%b", $time, y);
        #2 $display("t=%0t y=%b", $time, y);
        #2 $display("t=%0t y=%b", $time, y);
    end
endmodule
"#;

    // Hand-simulation (unit=10ns, precision=1ns → 1 unit = 10 ticks):
    //   t=0     CA suspends for scaled #2 = 20 ticks (= 2 units).
    //           src=1.
    //   t=10    (plain #1): display y=x (CA write due tick 20 — if the CA
    //           delay failed to scale, y would already be 1 here).
    //   t=30    (plain #2 after it): CA wrote y=1 at tick 20 → display y=1.
    //   t=50    display y=1.
    //   %t shows $time in the module's unit: ticks*1ns/10ns.
    //
    // Expected stdout (exactly):
    //   t=1 y=x
    //   t=3 y=1
    //   t=5 y=1
    let stdout = run_sim(sv, "cats").expect("simulation should run");
    assert_eq!(stdout, "t=1 y=x\nt=3 y=1\nt=5 y=1\n", "stdout: {stdout}");
}

/// (f) Zero-delay forms: `a = #0 rhs` lands through the inactive region of
/// the same time step, and `c <= #0 rhs` records an NBA that commits only
/// once the inactive region has fully drained — so even a subsequent `#0`
/// window still reads the old value, while a real delay later sees the
/// committed one.
#[test]
fn sim_intra_delay_zero_delay() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] a;
    reg c;

    initial begin
        a = #0 8'h5a;
        $display("t=%0t a=%h", $time, a);
        c <= #0 1'b1;
        $display("d1 c=%b", c);
        #0 $display("d2 c=%b", c);
        #1 $display("t=%0t c=%b", $time, c);
        #1 $finish;
    end
endmodule
"#;

    // Hand-simulation (IEEE 1800 §4: the inactive region drains every #0
    // continuation BEFORE the NBA region runs):
    //   active:           temp_a=5a; wait_time(0) reschedules inactive.
    //   inactive pass 1:  a := 5a; display "t=0 a=5a"; `c <= #0 1`
    //                     suspends into a further inactive pass.
    //   inactive pass 2:  RECORD NBA(c=1); display "d1 c=x" (recorded, not
    //                     yet committed); wait_time(0) again.
    //   inactive pass 3:  display "d2 c=x" — still pre-NBA, since all #0
    //                     continuations drain first; then #1 moves the
    //                     process to the timed queue, ending the drain.
    //   NBA region:       commit c=1.
    //   t=1:              display "t=1 c=1"; then #1 $finish.
    //
    // Expected stdout (exactly):
    //   t=0 a=5a
    //   d1 c=x
    //   d2 c=x
    //   t=1 c=1
    let stdout = run_sim(sv, "zero").expect("simulation should run");
    assert_eq!(
        stdout, "t=0 a=5a\nd1 c=x\nd2 c=x\nt=1 c=1\n",
        "stdout: {stdout}"
    );
}

/// (g) Event-controlled intra-assignment (`a = @(posedge clk) b;`) is
/// rejected at codegen with the documented error instead of being silently
/// degraded.
#[test]
fn sim_intra_delay_event_form_rejected() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg a, b, clk;

    initial begin
        clk = 1'b0;
        a = @(posedge clk) b;
    end
endmodule
"#;

    let result = codegen_result(sv, "evt").expect("compile should succeed");
    let err = match result {
        Ok(_) => panic!("codegen should reject event-controlled intra-assignment"),
        Err(e) => e,
    };
    assert!(
        err.contains("intra-assignment event/repeat control"),
        "unexpected codegen error: {err}"
    );
    assert!(
        err.contains("is not supported"),
        "unexpected codegen error: {err}"
    );
}

/// A prior delay on the same source line must not make an event-controlled
/// intra-assignment look like it carries that earlier `#` token.
#[test]
fn sim_intra_event_after_same_line_delay_rejected() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    for (tag, statement) in [
        ("prior", "#1; a = @(posedge clk) b;"),
        ("following", "a = @(posedge clk) b; #1 $finish;"),
        ("outer", "#1 a = @(posedge clk) b;"),
    ] {
        let sv = format!(
            r#"module tb;
    reg a, b, clk;
    initial begin {statement} end
endmodule
"#
        );
        let result =
            codegen_result(&sv, &format!("same_line_event_{tag}")).expect("compile should succeed");
        let error = match result {
            Ok(_) => panic!("event-controlled intra-assignment `{tag}` should be rejected"),
            Err(error) => error,
        };
        assert!(
            error.contains("intra-assignment event/repeat control"),
            "unexpected codegen error for `{tag}`: {error}"
        );
    }
}

/// (g) Repeat-form intra-assignment (`a = repeat(2) @(posedge clk) b;`)
/// produces the same clean rejection family as the event form.
#[test]
fn sim_intra_delay_repeat_form_rejected() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg a, b, clk;

    initial begin
        clk = 1'b0;
        a = repeat(2) @(posedge clk) b;
    end
endmodule
"#;

    let result = codegen_result(sv, "rep").expect("compile should succeed");
    let err = match result {
        Ok(_) => panic!("codegen should reject repeat-controlled intra-assignment"),
        Err(e) => e,
    };
    assert!(
        err.contains("intra-assignment event/repeat control"),
        "unexpected codegen error: {err}"
    );
}

/// Parameter and arithmetic-expression intra-assignment delays are folded
/// against the elaborated instance parameters before timescale scaling.
#[test]
fn sim_intra_delay_parameterized_expression() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg a, b;
    parameter P = 2;

    initial begin
        b = 1'b1;
        a = #P b;
        $display("t=%0t a=%b", $time, a);
        a = #(P + 1) 1'b0;
        $display("t=%0t a=%b", $time, a);
    end
endmodule
"#;

    let stdout = run_sim(sv, "par").expect("simulation should run");
    assert_eq!(stdout, "t=2 a=1\nt=5 a=0\n", "stdout: {stdout}");
}

/// A delay control starting on the line after its assignment operator keeps
/// using the delay-control object's exact source position.
#[test]
fn sim_intra_delay_multiline_literal() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg a, b;
    initial begin
        b = 1'b1;
        a =
            #2 b;
        $display("t=%0t a=%b", $time, a);
    end
endmodule
"#;

    let stdout = run_sim(sv, "multiline_intra_delay").expect("simulation should run");
    assert_eq!(stdout, "t=2 a=1\n", "stdout: {stdout}");
}

/// (g) Fractional and unit-suffixed intra-assignment delays retain their
/// complete source token and each RHS is captured before its 500ps delay.
#[test]
fn sim_intra_delay_fractional_and_unit_literals() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ps
module tb;
    reg [7:0] a, b;

    initial begin
        b = 8'd1;
        a = #0.5 b;
        b = 8'd2;
        a = #500ps b;
        $display("t=%0t a=%0d", $time, a);
    end
endmodule
"#;
    let stdout = run_sim(sv, "intra_fractional_literals").expect("simulation should run");
    assert_eq!(stdout, "t=1 a=2\n", "stdout: {stdout}");
}

/// (h) Statement delays accept both a bare fixed-point value in the calling
/// module's unit and an explicit physical time literal.
#[test]
fn sim_stmt_delay_fractional_and_unit_literals() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ps
module tb;
    reg [7:0] a;

    initial begin
        #0.5 a = 8'd7;
        #500ps a = 8'd9;
        $display("t=%0t a=%0d", $time, a);
    end
endmodule
"#;
    let stdout = run_sim(sv, "stmt_fractional_literals").expect("simulation should run");
    assert_eq!(stdout, "t=1 a=9\n", "stdout: {stdout}");
}

/// Procedural delay controls accept elaborated parameters, parenthesized
/// constant arithmetic, and underscore-separated decimal integers (Verilog
/// 1364-2001 §9.7.1).
#[test]
fn sim_stmt_delay_constant_expressions() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    parameter integer P = 2;
    parameter [31:0] WRAP = 32'hffff_ffff;

    initial begin
        #P $display("t=%0t parameter", $time);
        #(P * 2 + 1) $display("t=%0t expression", $time);
        #1_000 $display("t=%0t underscore", $time);
        #(WRAP + 1) $display("t=%0t width-wrap", $time);
    end
endmodule
"#;

    let stdout = run_sim(sv, "stmt_expr").expect("simulation should run");
    assert_eq!(
        stdout, "t=2 parameter\nt=7 expression\nt=1007 underscore\nt=1007 width-wrap\n",
        "stdout: {stdout}"
    );
}

/// Signed negative parameter delays are rejected before scale conversion.
#[test]
fn sim_stmt_negative_parameter_delay_rejected() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    parameter signed N = -1;
    initial #N $finish;
endmodule
"#;

    let result = codegen_result(sv, "negative_stmt_delay").expect("compile should succeed");
    let error = match result {
        Ok(_) => panic!("negative procedural delay should be rejected"),
        Err(error) => error,
    };
    assert!(
        error.contains("procedural delay must be a known nonnegative integer"),
        "unexpected codegen error: {error}"
    );
}

/// Nested mixed-width arithmetic must not be eagerly folded: Verilog widens
/// `(A+B)` from four to five bits through the outer `+ C`, preserving its
/// carry and producing a delay of 16.
#[test]
fn sim_stmt_mixed_width_delay_expression_runs_at_16() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    parameter logic [3:0] A = 15;
    parameter logic [3:0] B = 1;
    parameter logic [4:0] C = 0;
    initial begin
        #((A + B) + C) $display("t=%0t", $time);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "mixed_width_stmt_delay").expect("simulation should run");
    assert_eq!(stdout, "t=16\n");
}

/// A mixed-signedness outer expression can reinterpret an already-computed
/// child. With unsigned outer context, `4'sb1000 / 2` must be treated as
/// unsigned 8/2 rather than eagerly folded as signed -8/2.
#[test]
fn sim_stmt_nested_mixed_signedness_delay_runs_at_4() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    parameter logic signed [3:0] A = -8;
    parameter logic signed [3:0] B = 2;
    parameter logic        [3:0] C = 0;
    initial begin
        #((A / B) + C) $display("t=%0t", $time);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "mixed_sign_stmt_delay").expect("simulation should run");
    assert_eq!(stdout, "t=4\n");
}

/// Signal-dependent delay expressions need a runtime-valued delay IR and are
/// rejected explicitly by the current constant-expression path.
#[test]
fn sim_stmt_dynamic_delay_expression_rejected() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] delay;
    initial begin
        delay = 2;
        #(delay + 1) $finish;
    end
endmodule
"#;

    let result = codegen_result(sv, "dynamic_delay").expect("compile should succeed");
    let error = match result {
        Ok(_) => panic!("dynamic delay expression should be rejected"),
        Err(error) => error,
    };
    assert!(
        error.contains("procedural delay") && error.contains("runtime-valued"),
        "unexpected codegen error: {error}"
    );
}

/// (i) A negative parameter value folded into a continuous-assignment delay
/// (`assign #P …` with `parameter signed P = -2;`) carries its
/// two's-complement bit pattern; it must be rejected instead of wrapping
/// into a huge tick count.
#[test]
fn sim_ca_delay_negative_parameter_rejected() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    parameter signed P = -2;
    reg src;
    wire y;

    assign #P y = src;

    initial begin
        src = 1'b1;
        #1 $display("t=%0t y=%b", $time, y);
    end
endmodule
"#;

    let result = codegen_result(sv, "negca").expect("compile should succeed");
    match result {
        Ok(_) => panic!("codegen should reject negative continuous-assignment delays"),
        Err(e) => assert!(
            e.contains("procedural delay must be a known nonnegative integer"),
            "unexpected codegen error: {e}"
        ),
    }
}

/// (j) A statement delay whose timescale-scaled tick product overflows the
/// u64 tick range (`#1000000000` in a `1s/1ps` module → 10^21 ticks) is
/// rejected instead of silently truncating through the `as u64` cast.
#[test]
fn sim_stmt_delay_scaling_overflow_rejected() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1s/1ps
module tb;
    reg [7:0] a;

    initial begin
        #1000000000 a = 8'd7;
        $display("t=%0t a=%0d", $time, a);
    end
endmodule
"#;

    let result = codegen_result(sv, "ovf").expect("compile should succeed");
    match result {
        Ok(_) => panic!("codegen should reject delays that overflow the tick range"),
        Err(e) => assert!(
            e.contains("scales past the 64-bit tick range"),
            "unexpected codegen error: {e}"
        ),
    }
}

/// (h) Optimizer parity: a mixed delayed-assignment design produces
/// byte-identical stdout with all optimization passes on and off.
#[test]
fn sim_delay_opt_parity() {
    use llg::sim::opt::OptConfig;

    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    reg [7:0] a, b;
    reg src;
    wire [3:0] y;

    assign #2 y = {3'b000, src};

    initial begin
        b = 8'd1;
        a = #5 b;
        b = 8'd2;
        $display("t=%0t a=%0d b=%0d y=%0d", $time, a, b, y);
        src = 1'b1;
        #4 $display("t=%0t y=%0d", $time, y);
        #2 $display("t=%0t y=%0d", $time, y);
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  b=1; `a=#5 b` captures b==1 and suspends; CA suspends for #2
    //        (its first evaluation reads nothing yet).
    //   t=2  CA writes y = current src = x (src is untouched so far).
    //   t=5  a:=1; b=2; display "t=5 a=1 b=2 y=x"; src=1 wakes the CA
    //        (#2 window open until t=7).
    //   t=7  CA writes y = current src = 1.
    //   t=9  display y=1.
    //   t=11 display y=1.
    //
    // Expected stdout (both optimizer settings, byte-identical):
    //   t=5 a=1 b=2 y=x
    //   t=9 y=1
    //   t=11 y=1

    fn build_and_run(
        dir: &std::path::Path,
        db: &llg::core::db::Db,
        cfg: &OptConfig,
    ) -> Result<String, String> {
        let gen =
            sim::codegen::generate_from_db_with_opts(db, cfg).map_err(|error| error.to_string())?;
        let exe = sim::build::build_model_cmake_with_opts(
            dir,
            &[("model.c", gen.model_c.as_str())],
            &Default::default(),
        )
        .map_err(|e| format!("cmake: {e}"))?;
        sim_harness::run_executable(&exe)
    }

    // Compile once and lower the same owned frontend snapshot with both
    // optimizer configurations; recompilation is outside the behavior this
    // differential test compares.
    let (on, off) = sim_harness::with_frontend_temp_cwd("delay-opt", |dir| {
        let src_path = dir.join("tb.sv");
        std::fs::write(&src_path, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile_checked(&compile::CompileOpts {
            files: vec![src_path.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"));
        Ok(match out {
            Ok(out) => {
                let database =
                    llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| error.to_string());
                match database {
                    Ok(database) => (
                        build_and_run(dir, &database, &OptConfig::default()),
                        build_and_run(dir, &database, &OptConfig::none()),
                    ),
                    Err(error) => (Err(error.to_string()), Err(error.to_string())),
                }
            }
            Err(error) => {
                let message = error.to_string();
                (Err(message.clone()), Err(message))
            }
        })
    })
    .expect("delay parity setup");

    let expected = "t=5 a=1 b=2 y=x\nt=9 y=1\nt=11 y=1\n";
    assert_eq!(on.expect("opt-on run"), expected);
    assert_eq!(off.expect("opt-off run"), expected);
}
