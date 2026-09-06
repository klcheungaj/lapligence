//! End-to-end simulator tests for function/task support: Surelog compile →
//! codegen → CMake build → run, asserting exact stdout against hand-simulated
//! traces.
//!
//! Regression coverage for the function/task fixes: empty bodies, non-blocking
//! writes to output formals, function-call sensitivity for combinational
//! processes, formal defaults referencing earlier formals (codegen + elab),
//! writes to input formals, and assignment-like width contexts (§10.8).
//!
//! Surelog writes `slpp_all/` into the process working directory, so the
//! tests run with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, like the other Surelog integration tests).

use std::{path::Path, sync::Mutex};

use llg::core::elab;
use llg::core::{compile, db::Db};
use llg::sim;
use llg::sim::opt::OptConfig;

#[path = "support/sim.rs"]
mod sim_harness;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Compile + codegen + C-compile + run `sv` (top module `top`), returning the
/// simulator's exact stdout and the codegen warnings.
fn run_sim(sv: &str, top: &str, tag: &str) -> Result<(String, Vec<String>), String> {
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
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        let gen = sim::codegen::generate(design).map_err(|e| format!("codegen: {e}"))?;
        let exe = sim::build::build_model_cmake(dir, &[("model.c", gen.model_c.as_str())])
            .map_err(|e| format!("cmake: {e}"))?;
        let stdout = sim_harness::run_executable(&exe)?;
        Ok((stdout, gen.warnings))
    })
}

fn fixture_rejection(file: &str, tag: &str) -> Result<String, String> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/function")
        .join(file);
    sim_harness::with_temp_cwd(tag, |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile fixture: {error}"))?;
        if !compiled.ok() {
            return Ok(format!("{:?}", compiled.diagnostics));
        }
        match sim::codegen::generate(compiled.uhdm_design().ok_or("no UHDM design")?) {
            Ok(_) => Err(format!("{file} unexpectedly generated")),
            Err(error) => Ok(error.to_string()),
        }
    })
}

fn run_fixture_both_opts(file: &str, tag: &str, expected: &str) -> Result<(), String> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/function")
        .join(file);
    sim_harness::with_temp_cwd(tag, |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile fixture: {error}"))?;
        if !compiled.ok() {
            return Err(format!("frontend diagnostics: {:?}", compiled.diagnostics));
        }
        let database = Db::build_with_source_files(
            compiled.uhdm_design().ok_or("no UHDM design")?,
            &compiled.frontend_source_files(),
        )
        .map_err(|error| format!("database: {error}"))?;

        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map_err(|error| format!("{variant} codegen: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} cmake: {error}"))?;
            let actual = sim_harness::run_executable(&executable)?;
            if actual != expected {
                return Err(format!("{variant}: expected {expected:?}, got {actual:?}"));
            }
        }
        Ok(())
    })
}

/// (a) Recursive function: `fact(5)` must return 120.  The function-name
/// variable is both written (return value) and read (base case), and the
/// recursion guard must not trip on a bounded depth.
#[test]
fn sim_func_recursive_factorial() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    logic clk;
    logic [7:0] out;

    function automatic logic [7:0] fact(input logic [7:0] x);
        if (x <= 1)
            fact = 8'd1;
        else
            fact = x * fact(x - 1);
    endfunction

    always #5 clk = ~clk;

    always @(posedge clk) begin
        out <= fact(8'd5);
    end

    always @(posedge clk) begin
        if (out !== 8'bx) $display("t=%0t fact(5)=%0d", $time, out);
    end

    initial begin
        clk = 0;
        #30 $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  clk=0; the posedge processes register waiters, initial #30.
    //   t=5  clk 0->1 posedge: first proc records out<=fact(5)=120; second
    //        proc sees out=X (NBA not committed): `out !== 8'bx` is truthy
    //        against the all-X out (the X literal is a single x bit), so it
    //        prints "t=5 fact(5)=x".  NBA: out=120.
    //   t=15 posedge: out<=120; second proc: out=120 -> "t=15 fact(5)=120".
    //   t=25 posedge: "t=25 fact(5)=120".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   t=5 fact(5)=x
    //   t=15 fact(5)=120
    //   t=25 fact(5)=120

    let (stdout, _warnings) = run_sim(sv, "tb", "fact").expect("simulation should run");
    assert_eq!(
        stdout,
        "t=5 fact(5)=x\nt=15 fact(5)=120\nt=25 fact(5)=120\n"
    );
}

/// (b) Empty function body used as a statement: the definition must codegen
/// into a no-op C function (Surelog emits no `vpiStmt` for an empty body).
#[test]
fn sim_func_empty_body_stmt() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    logic clk;
    logic [3:0] out;

    function void nop(input logic [3:0] x);
    endfunction

    always #5 clk = ~clk;

    always @(posedge clk) begin
        nop(4'd1);
        out <= 4'd3;
    end

    always @(posedge clk) begin
        if (out !== 4'bx) $display("t=%0t out=%0d", $time, out);
    end

    initial begin
        clk = 0;
        #30 $finish;
    end
endmodule
"#;

    // Hand-simulation (mirrors sim_func_recursive_factorial):
    //   t=5  posedge: nop() no-op; out<=3 recorded; the guarded display sees
    //        out=X (NBA not committed) and prints "t=5 out=x".  NBA: out=3.
    //   t=15 posedge: out<=3; display "t=15 out=3".
    //   t=25 posedge: display "t=25 out=3".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   t=5 out=x
    //   t=15 out=3
    //   t=25 out=3

    let (stdout, _warnings) = run_sim(sv, "tb", "empty").expect("simulation should run");
    assert_eq!(stdout, "t=5 out=x\nt=15 out=3\nt=25 out=3\n");
}

/// (c) Task with an output formal, blocking write.  The output actual is a
/// whole signal, so the write must land on it (via the writeback).
#[test]
fn sim_task_output_blocking() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    logic clk;
    logic [3:0] q;
    logic [3:0] d;

    task add_one(input logic [3:0] x, output logic [3:0] y);
        y = x + 1;
    endtask

    always #5 clk = ~clk;

    always @(posedge clk) begin
        d <= 4'd7;
        add_one(d, q);
        $display("t=%0t d=%0d q=%0d", $time, d, q);
    end

    initial begin
        clk = 0;
        #30 $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  clk=0.
    //   t=5  posedge: d<=7 recorded (NBA, not yet visible); add_one reads
    //        d=X -> y = X+1 = X, written to q; display "t=5 d=x q=x".
    //        NBA: d=7.
    //   t=15 posedge: add_one(d=7) -> q = 8 (blocking); display "t=15 d=7 q=8".
    //   t=25 posedge: d=7 still; q=8; display "t=25 d=7 q=8".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   t=5 d=x q=x
    //   t=15 d=7 q=8
    //   t=25 d=7 q=8

    let (stdout, _warnings) = run_sim(sv, "tb", "taskblk").expect("simulation should run");
    assert_eq!(stdout, "t=5 d=x q=x\nt=15 d=7 q=8\nt=25 d=7 q=8\n");
}

/// (d) A static task output formal retains the value committed by its prior
/// NBA. Copy-out on the next return must then update the caller's actual.
#[test]
fn sim_task_output_nba() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/function/task_output_nba.sv");
    let source = std::fs::read_to_string(&fixture).expect("read task output NBA fixture");
    let (stdout, _warnings) = run_sim(&source, "tb", "tasknba").expect("simulation should run");
    assert_eq!(stdout, "PASS task_output_nba\n");
}

#[test]
fn sim_task_nba_static_input_and_local_targets_persist() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let cases = [
        (
            "task_nba_input_formal.sv",
            "task_nba_input_formal",
            "PASS task_nba_input_formal\n",
        ),
        (
            "task_nba_local_storage.sv",
            "task_nba_local_storage",
            "PASS task_nba_local_storage\n",
        ),
    ];
    for (file, tag, expected) in cases {
        run_fixture_both_opts(file, tag, expected)
            .unwrap_or_else(|error| panic!("{file} must execute correctly: {error}"));
    }
}

#[test]
fn sim_task_nba_rejects_automatic_input_and_local_targets() {
    let _guard = SURELOG_LOCK.lock().unwrap();
    for (file, tag) in [
        (
            "task_nba_automatic_input_formal.sv",
            "task_nba_automatic_input_formal",
        ),
        (
            "task_nba_automatic_local_storage.sv",
            "task_nba_automatic_local_storage",
        ),
    ] {
        let error = fixture_rejection(file, tag)
            .unwrap_or_else(|error| panic!("{file} must be explicitly rejected: {error}"));
        let normalized = error.to_ascii_lowercase();
        assert!(
            normalized.contains("nonblocking")
                && normalized.contains("task")
                && (normalized.contains("automatic")
                    || (normalized.contains("stack-backed")
                        && normalized.contains("cannot outlive"))),
            "{file}: unexpected diagnostic: {error}"
        );
    }
}

/// (e) always_comb calling a function that reads a module signal (regression:
/// the sensitivity walk used to miss reads inside the callee, so `out` never
/// tracked `a` and a spurious "reads no signals" warning fired).
#[test]
fn sim_func_comb_sensitivity() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    logic [3:0] a;
    logic [3:0] out;
    logic [3:0] out2;

    function automatic logic [3:0] reada();
        reada = a;
    endfunction

    always_comb out = reada();
    always_comb out2 = a;

    initial begin
        a = 0;
        #3 a = 4'd2;
        #2 a = 4'd3;
        #1 $display("t=%0t a=%0d out=%0d out2=%0d", $time, a, out, out2);
        #9 $display("t=%0t a=%0d out=%0d out2=%0d", $time, a, out, out2);
        $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  comb(out) evaluates reada()=a=X -> out=X, waits on {a};
    //        comb(out2) -> out2=X, waits on {a}; initial: a=0 (X->0) wakes
    //        both combs; combs re-run: out=0, out2=0.  #3.
    //   t=3  initial: a=2 -> combs re-run: out=2, out2=2.  #2.
    //   t=5  initial: a=3 -> combs re-run: out=3, out2=3.  #1.
    //   t=6  display "t=6 a=3 out=3 out2=3".  #9.
    //   t=15 display "t=15 a=3 out=3 out2=3"; $finish.
    //
    // Expected stdout (exactly):
    //   t=6 a=3 out=3 out2=3
    //   t=15 a=3 out=3 out2=3

    let (stdout, warnings) = run_sim(sv, "tb", "combsens").expect("simulation should run");
    assert_eq!(stdout, "t=6 a=3 out=3 out2=3\nt=15 a=3 out=3 out2=3\n");
    assert!(
        !warnings.iter().any(|w| w.contains("reads no signals")),
        "unexpected warnings: {warnings:?}"
    );
}

/// (f) A formal default that references an earlier formal (`b = a + 1`):
/// defaults are emitted in a formal-aware context so `a` resolves to the
/// bound argument (regression: the default used to resolve in the caller's
/// scope and failed).
#[test]
fn sim_func_default_refs_earlier_formal() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    logic clk;
    logic [7:0] out;

    function automatic logic [7:0] f(input logic [7:0] a = 8'd1, input logic [7:0] b = a + 8'd1);
        f = a * 10 + b;
    endfunction

    always #5 clk = ~clk;

    always @(posedge clk) begin
        out <= f();
    end

    always @(posedge clk) begin
        if (out !== 8'bx) $display("t=%0t out=%0d", $time, out);
    end

    initial begin
        clk = 0;
        #30 $finish;
    end
endmodule
"#;

    // Hand-simulation: f() binds a=1 (default), b=a+1=2 (default referencing
    // the earlier formal) -> f = 1*10 + 2 = 12.
    //
    //   t=0  clk=0.
    //   t=5  posedge: out<=12 recorded; second proc sees out=X (NBA not
    //        committed) and prints "t=5 out=x".  NBA: out=12.
    //   t=15 posedge: "t=15 out=12".
    //   t=25 posedge: "t=25 out=12".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   t=5 out=x
    //   t=15 out=12
    //   t=25 out=12

    let (stdout, _warnings) = run_sim(sv, "tb", "defref").expect("simulation should run");
    assert_eq!(stdout, "t=5 out=x\nt=15 out=12\nt=25 out=12\n");
}

/// (f-elab) The elab twin: `Resolver::eval_expr` on the `f()` call must bind
/// the formals one at a time so `b`'s default (`b = a + 1`) sees `a` in
/// scope.  (Constant calls in `localparam` initializers are inlined away by
/// Surelog's own elaborator, so the call is taken from a process body where
/// the UHDM keeps the `func_call` node.)
#[test]
fn sim_func_default_elab_resolver() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    function automatic logic [7:0] f(input logic [7:0] a = 8'd1, input logic [7:0] b = a + 8'd1);
        f = a * 10 + b;
    endfunction
    logic [7:0] out;
    always_comb out = f();
endmodule
"#;
    let result = sim_harness::with_temp_cwd("function-default-elab", |dir| {
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
        use llg::ffi::vpi;
        let top = vpi::iterate(vpi::uhdmtopModules, design)
            .ok_or("no top modules")?
            .next()
            .ok_or("no top module")?;
        let proc = vpi::iterate(vpi::vpiProcess, top.raw())
            .ok_or("no process")?
            .next()
            .ok_or("no process")?;
        // always_comb with a single assignment: the process statement *is* the
        // assignment.  Keep the owned handles alive while their raw pointers
        // are in use.
        let stmt_owned = vpi::handle(vpi::vpiStmt, proc.raw()).ok_or("no process stmt")?;
        let rhs_owned = vpi::handle(vpi::vpiRhs, stmt_owned.raw()).ok_or("no RHS")?;
        let mut resolver = elab::Resolver::new();
        resolver
            .eval_expr(top.raw(), rhs_owned.raw())
            .map_err(|e| format!("eval: {e}"))
    });

    // f() binds a=1 (default), b=a+1=2 (default referencing the earlier
    // formal) -> f = 1*10 + 2 = 12.
    let val = result.expect("eval_expr should resolve f()");
    let elab::Val::Bits(v) = val else {
        panic!("expected bits, got {val:?}");
    };
    assert_eq!(v.to_u64().expect("no X/Z"), 12, "f() must evaluate to 12");
}

/// (g) Writing an input formal (legal SV — it is a local copy): the by-value
/// C parameter must be a writable lvalue in the generated function.
#[test]
fn sim_func_write_input_formal() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    logic clk;
    logic [7:0] out;

    function automatic logic [7:0] f(input logic [7:0] x);
        x = x + 8'd1;
        f = x;
    endfunction

    always #5 clk = ~clk;

    always @(posedge clk) begin
        out <= f(8'd41);
    end

    always @(posedge clk) begin
        if (out !== 8'bx) $display("t=%0t out=%0d", $time, out);
    end

    initial begin
        clk = 0;
        #30 $finish;
    end
endmodule
"#;

    // Hand-simulation: f(41) increments its input copy: x=42, f=42.
    //
    //   t=0  clk=0.
    //   t=5  posedge: out<=42 recorded; second proc sees out=X and prints
    //        "t=5 out=x".  NBA: out=42.
    //   t=15 posedge: "t=15 out=42".
    //   t=25 posedge: "t=25 out=42".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   t=5 out=x
    //   t=15 out=42
    //   t=25 out=42

    let (stdout, _warnings) = run_sim(sv, "tb", "wrin").expect("simulation should run");
    assert_eq!(stdout, "t=5 out=x\nt=15 out=42\nt=25 out=42\n");
}

/// (h) Delay-bearing task inlined at its call site: the caller's `$display`
/// runs after the inlined `#5` completes, so the timing shows through.
#[test]
fn sim_task_delay_inlined() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    logic clk;
    logic [3:0] out;
    logic [3:0] v;

    task delay_inc(input logic [3:0] val, output logic [3:0] o);
        o = val;
        #5 o = o + 1;
    endtask

    always #5 clk = ~clk;

    always @(posedge clk) begin
        v <= 4'd1;
        delay_inc(v, out);
        $display("t=%0t after-call out=%0d", $time, out);
    end

    initial begin
        clk = 0;
        #26 $finish;
    end
endmodule
"#;

    // Hand-simulation:
    //   t=0  clk=0.
    //   t=5  posedge: v<=1 recorded; inlined delay_inc: out = v = X; #5
    //        suspends the process until t=10.  NBA: v=1.
    //   t=10 resume: out = X+1 = X; display "t=10 after-call out=x".
    //   t=15 posedge: v<=1 (same); inlined: out = 1; #5 -> t=20.
    //   t=20 resume: out = 1+1 = 2; display "t=20 after-call out=2".
    //   t=25 posedge: inlined: out = 1; #5 -> t=30 (never resumes: the
    //        initial $finish fires at t=26 first).
    //
    // Expected stdout (exactly):
    //   t=10 after-call out=x
    //   t=20 after-call out=2

    let (stdout, _warnings) = run_sim(sv, "tb", "taskdelay").expect("simulation should run");
    assert_eq!(stdout, "t=10 after-call out=x\nt=20 after-call out=2\n");
}

/// A subroutine input argument is an assignment-like context (IEEE 1800-2009
/// §10.8).  The 16-bit formal must widen the 8-bit operands before addition;
/// widening only after the 8-bit sum would pass zero instead of 256.
#[test]
fn sim_func_input_argument_assignment_context() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    function automatic logic [15:0] accept(input logic [15:0] value);
        accept = value;
    endfunction

    initial begin
        $display("arg=%0d", accept(8'hff + 8'h1));
        $finish;
    end
endmodule
"#;

    let (stdout, _warnings) = run_sim(sv, "tb", "argctx").expect("simulation should run");
    assert_eq!(stdout, "arg=256\n");
}

/// A function's return value is an assignment-like context (IEEE 1800-2009
/// §10.8).  Its 16-bit return type must widen the 8-bit operands before
/// addition, preserving the 16'h0100 result.
#[test]
fn sim_func_return_assignment_context() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    function automatic logic [15:0] add_wide();
        add_wide = 8'hff + 8'h1;
    endfunction

    initial begin
        $display("ret=%0d", add_wide());
        $finish;
    end
endmodule
"#;

    let (stdout, _warnings) = run_sim(sv, "tb", "retctx").expect("simulation should run");
    assert_eq!(stdout, "ret=256\n");
}
