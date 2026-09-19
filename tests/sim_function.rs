//! End-to-end simulator tests for function/task support: Slang compile →
//! codegen → CMake build → run, asserting exact stdout against hand-simulated
//! traces.
//!
//! Regression coverage for the function/task fixes: empty bodies, non-blocking
//! writes to output formals, function-call sensitivity for combinational
//! processes, formal defaults referencing earlier formals (semantic DB + codegen),
//! writes to input formals, and assignment-like width contexts (§10.8).
//!
//! These tests temporarily change the process working directory, so the
//! tests run with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, to avoid process-wide CWD races).

use std::{path::Path, sync::Mutex};

use llg::core::{compile, db::Db};
use llg::sim;
use llg::sim::opt::OptConfig;

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

static CWD_LOCK: Mutex<()> = Mutex::new(());

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
        let db = Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
        let gen = sim::codegen::generate(&db).map_err(|e| format!("codegen: {e}"))?;
        let exe = sim::build::build_model_cmake(dir, &[("model.c", gen.model_c.as_str())])
            .map_err(|e| format!("cmake: {e}"))?;
        let stdout = sim_harness::run_executable(&exe)?;
        Ok((stdout, gen.warnings))
    })
}

fn run_source_both_opts(sv: &str, top: &str, tag: &str, expected: &str) -> Result<(), String> {
    sim_harness::with_temp_cwd(tag, |dir| {
        let source = dir.join("tb.sv");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some(top.to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        if !compiled.ok() {
            return Err(format!("frontend diagnostics: {:?}", compiled.diagnostics));
        }
        let database =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;
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
        let db =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;
        match sim::codegen::generate(&db) {
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
        let database =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;

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
    let _guard = CWD_LOCK.lock().unwrap();
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
    //        proc sees out=X (NBA not committed). Both operands of
    //        `out !== 8'bx` are all X, so the guard is false. NBA: out=120.
    //   t=15 posedge: out<=120; second proc: out=120 -> "t=15 fact(5)=120".
    //   t=25 posedge: "t=25 fact(5)=120".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   t=15 fact(5)=120
    //   t=25 fact(5)=120

    let (stdout, _warnings) = run_sim(sv, "tb", "fact").expect("simulation should run");
    assert_eq!(stdout, "t=15000 fact(5)=120\nt=25000 fact(5)=120\n");
}

/// (b) Empty function body used as a statement: the definition must codegen
/// into a no-op C function.
#[test]
fn sim_func_empty_body_stmt() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
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
    //        out=4'bxxxx before the NBA. Case inequality compares X bits
    //        exactly (IEEE 1800-2009 §11.4.5), so `out !== 4'bx` is false
    //        and nothing prints. NBA: out=3.
    //   t=15 posedge: out<=3; display "t=15 out=3".
    //   t=25 posedge: display "t=25 out=3".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   t=15 out=3
    //   t=25 out=3

    let (stdout, _warnings) = run_sim(sv, "tb", "empty").expect("simulation should run");
    assert_eq!(stdout, "t=15000 out=3\nt=25000 out=3\n");
}

/// (c) Task with an output formal, blocking write.  The output actual is a
/// whole signal, so the write must land on it (via the writeback).
#[test]
fn sim_task_output_blocking() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
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
    assert_eq!(stdout, "t=5000 d=x q=x\nt=15000 d=7 q=8\nt=25000 d=7 q=8\n");
}

/// (d) A static task output formal retains the value committed by its prior
/// NBA. Copy-out on the next return must then update the caller's actual.
#[test]
fn sim_task_output_nba() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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
fn sim_static_function_output_inout_expression_copyout_and_real() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    run_fixture_both_opts(
        "static_function_output_inout_expr.sv",
        "static_function_output_inout_expr",
        "packed=6 side=5 inout=8 side2=8 real=3.5 rside=2.5\n",
    )
    .expect("static function output/inout expression copy-out must match in both modes");
}

#[test]
fn sim_subroutine_inout_selected_actual_evaluates_index_once() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    run_fixture_both_opts(
        "subroutine_inout_index_once.sv",
        "subroutine_inout_index_once",
        "value=5 index_calls=1\n",
    )
    .expect("inout selected actual must bind its index once in both modes");
}

#[test]
fn sim_disabled_delayed_task_skips_output_copyout() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    run_fixture_both_opts(
        "disabled_delayed_output_copyout.sv",
        "disabled_delayed_output_copyout",
        "PASS disabled_delayed_output_copyout\n",
    )
    .expect("disabled delayed tasks must not run output copy-out");
}

#[test]
fn sim_recursive_timed_task_uses_independent_activations() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    run_fixture_both_opts(
        "recursive_timed_task.sv",
        "recursive_timed_task",
        "recursive result=3 t=3000\n",
    )
    .expect("recursive timed task must preserve each activation and copy-out");
}

#[test]
fn sim_mutual_recursive_timed_tasks_suspend_and_return() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    run_fixture_both_opts(
        "mutual_recursive_timed_tasks.sv",
        "mutual_recursive_timed_tasks",
        "mutual result=1 t=4000\n",
    )
    .expect("mutually recursive timed tasks must share the typed call ABI");
}

#[test]
fn sim_timed_task_nested_fork_join_variants() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    run_fixture_both_opts(
        "timed_task_fork_joins.sv",
        "timed_task_fork_joins",
        "fork result=11 t=2000\n",
    )
    .expect("timed task fork/join variants must retain caller state");
}

#[test]
fn sim_timed_task_ref_alias_survives_suspension() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    run_fixture_both_opts(
        "timed_task_ref_alias.sv",
        "timed_task_ref_alias",
        "ref value=42 t=1000\n",
    )
    .expect("ref actuals must remain aliases while a task is suspended");
}

#[test]
fn sim_task_nba_rejects_automatic_input_and_local_targets() {
    let _guard = CWD_LOCK.lock().unwrap();
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

#[test]
fn sim_string_const_ref_mutation_is_rejected() {
    let _guard = CWD_LOCK.lock().unwrap();
    let error = fixture_rejection(
        "reference_string_const_mutation_rejected.sv",
        "reference_string_const_mutation_rejected",
    )
    .expect("string const-ref mutation must be explicitly rejected");
    let normalized = error.to_ascii_lowercase();
    assert!(
        normalized.contains("const")
            && normalized.contains("string")
            && (normalized.contains("mutat") || normalized.contains("writ")),
        "unexpected diagnostic: {error}"
    );
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    assert_eq!(
        stdout,
        "t=6000 a=3 out=3 out2=3\nt=15000 a=3 out=3 out2=3\n"
    );
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    //        committed). Both operands of `out !== 8'bx` are all X, so the
    //        guard is false. NBA: out=12.
    //   t=15 posedge: "t=15 out=12".
    //   t=25 posedge: "t=25 out=12".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   t=15 out=12
    //   t=25 out=12

    let (stdout, _warnings) = run_sim(sv, "tb", "defref").expect("simulation should run");
    assert_eq!(stdout, "t=15000 out=12\nt=25000 out=12\n");
}

/// (f-semantic) The owned semantic DB must retain the call target and both
/// formal defaults, including `b = a + 1` which refers to the earlier formal.
#[test]
fn sim_func_default_elab_resolver() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
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
        let database = Db::from_slang(&out.snapshot).map_err(|error| error.to_string())?;
        let callee = database
            .node_ids()
            .find_map(|id| match database.node_kind(id) {
                llg::core::db::NodeKind::FuncCall {
                    name,
                    callee: Some(callee),
                    ..
                } if name == "f" => Some(*callee),
                _ => None,
            })
            .ok_or("missing bound call to f")?;
        let defaults: Vec<_> = database
            .node(callee)
            .children
            .iter()
            .filter_map(|child| match database.node_kind(*child) {
                llg::core::db::NodeKind::FuncArg { default, .. } => Some(default.is_some()),
                _ => None,
            })
            .collect();
        Ok(defaults)
    });

    assert_eq!(result.expect("capture function defaults"), [true, true]);
}

/// (g) Writing an input formal (legal SV — it is a local copy): the by-value
/// C parameter must be a writable lvalue in the generated function.
#[test]
fn sim_func_write_input_formal() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
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
    //   t=5  posedge: out<=42 recorded; second proc sees out=X. Both operands
    //        of `out !== 8'bx` are all X, so the guard is false. NBA: out=42.
    //   t=15 posedge: "t=15 out=42".
    //   t=25 posedge: "t=25 out=42".
    //   t=30 $finish.
    //
    // Expected stdout (exactly):
    //   t=15 out=42
    //   t=25 out=42

    let (stdout, _warnings) = run_sim(sv, "tb", "wrin").expect("simulation should run");
    assert_eq!(stdout, "t=15000 out=42\nt=25000 out=42\n");
}

/// (h) Delay-bearing task inlined at its call site: the caller's `$display`
/// runs after the inlined `#5` completes, so the timing shows through.
#[test]
fn sim_task_delay_inlined() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
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
    assert_eq!(
        stdout,
        "t=10000 after-call out=x\nt=20000 after-call out=2\n"
    );
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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

#[test]
fn sim_hierarchical_subroutine_instances_and_parent_dispatch() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"module child #(parameter integer OFFSET = 0);
    integer state = 0;

    task bump(input integer amount, output integer result);
        state = state + amount + OFFSET;
        result = state;
    endtask

    function integer read(input integer amount);
        read = state + amount + OFFSET;
    endfunction

    initial begin
        #1 top.parent_bump(3);
    end
endmodule

module top;
    integer state = 0;
    integer a;
    integer b;
    child #(1) c0();
    child #(10) c1();

    task parent_bump(input integer amount);
        state = state + amount;
    endtask

    initial begin
        c0.bump(1, a);
        c1.bump(2, b);
        #2 $display("hier=%0d,%0d,%0d,%0d,%0d", state, a, b,
                    c0.read(3), c1.read(3));
        $finish;
    end
endmodule
"#;

    run_source_both_opts(
        sv,
        "top",
        "hierarchical-subroutine-instances",
        "hier=6,2,12,6,25\n",
    )
    .expect("hierarchical and per-instance subroutine dispatch should agree");
}

#[test]
fn sim_package_subroutine_state_is_shared_across_users() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"package counter_pkg;
    integer count = 0;
    function integer add(input integer delta);
        count = count + delta;
        add = count;
    endfunction
endpackage

module user #(parameter integer DELTA = 1)(output integer result);
    initial result = counter_pkg::add(DELTA);
endmodule

module top;
    integer first;
    integer second;
    user #(1) u0(first);
    user #(2) u1(second);

    initial begin
        #1 $display("pkg=%0d,%0d", first, second);
        $finish;
    end
endmodule
"#;

    run_source_both_opts(sv, "top", "package-subroutine-state", "pkg=1,3\n")
        .expect("package subroutine state should be shared");
}

#[test]
fn sim_package_runtime_state_initialization_and_imports() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    run_fixture_both_opts(
        "package_runtime_state.sv",
        "package-runtime-state",
        "pkg=4,4,1;4,6,2;import=4;export=4;unit=7;mirror=6\nmirror-after=7\n",
    )
    .expect("package state, initialization, static task storage, and imports should agree");
}

#[test]
fn sim_package_wildcard_import_ambiguity_remains_a_frontend_diagnostic() {
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"package left_pkg;
    integer value;
endpackage

package right_pkg;
    integer value;
endpackage

module tb;
    import left_pkg::*;
    import right_pkg::*;
    integer observed;
    initial observed = value;
endmodule
"#;
    sim_harness::with_temp_cwd("package-wildcard-ambiguity", |dir| {
        let source = dir.join("tb.sv");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        if compiled.ok() {
            return Err("ambiguous wildcard import unexpectedly compiled".to_owned());
        }
        let diagnostics = compiled
            .diagnostics
            .iter()
            .map(|diagnostic| format!("{diagnostic:?}"))
            .collect::<Vec<_>>()
            .join("\n");
        if !diagnostics.to_ascii_lowercase().contains("ambig") {
            return Err(format!(
                "wildcard import diagnostic was not ambiguity-specific: {diagnostics}"
            ));
        }
        Ok(())
    })
    .expect("frontend should retain wildcard-import ambiguity diagnostics");
}

#[test]
fn sim_interface_modport_subroutine_uses_parameterized_instance() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let sv = r#"interface channel #(parameter integer W = 4);
    logic [W-1:0] data;

    task set(input logic [W-1:0] value);
        data = value;
    endtask

    modport master(import task set(input logic [W-1:0] value), output data);
endinterface

module user #(parameter integer VALUE = 1)(channel.master ch);
    initial ch.set(VALUE);
endmodule

module top;
    channel #(4) c0();
    channel #(8) c1();
    user #(5) u0(c0);
    user #(9) u1(c1);

    initial begin
        #1 $display("if=%0d,%0d", c0.data, c1.data);
        $finish;
    end
endmodule
"#;

    run_source_both_opts(sv, "top", "interface-modport-subroutine", "if=5,9\n")
        .expect("interface modport subroutine dispatch should preserve widths");
}

/// G1-17: default arguments are evaluated only when omitted and output/inout
/// copy-out happens once, at return.
#[test]
fn sim_fixed_call_defaults_copyout() {
    sim_cli::run_case(
        "feature_completion/g1_17",
        "fixed_call_defaults_copyout",
        "defaults=1 y=5\n\
         defaults=1 s=15 c=107\n\
         defaults=1 s=11 c=112\n\
         inside=7\n\
         inside2=7\n\
         after=99\n",
        "",
        &[],
    );
}

/// A dependent default reads the earlier input after its one evaluation.
#[test]
fn sim_dependent_default_captures_prior_input_once() {
    sim_cli::run_case(
        "feature_completion/g1_17",
        "ref_default_side_effect_rejected",
        "calls=1 y=3\n",
        "",
        &[],
    );
}

/// G1-17: recursive automatic activations keep independent locals and return
/// slots.
#[test]
fn sim_automatic_recursive_function() {
    sim_cli::run_case(
        "feature_completion/g1_17",
        "automatic_recursive_function",
        "sum=60\n",
        "",
        &[],
    );
}

/// G1-15/G1-17: a fixed packed struct formal keeps per-activation member
/// storage, copy-in for inputs and copy-out at return for output/inout.
#[test]
fn sim_packed_struct_formal_abi() {
    sim_cli::run_case(
        "feature_completion/g1_17",
        "packed_struct_formal_abi",
        "mix=15 src=1005 acc=2007 out=2007\n\
         fill=7a00\n\
         combine=1005,1005 src=1005\n",
        "",
        &[],
    );
}

/// G1-15/G1-17: packed union members overlay one formal activation value,
/// including a nested packed struct member.
#[test]
fn sim_packed_union_formal_overlay() {
    sim_cli::run_case(
        "feature_completion/g1_17",
        "packed_union_formal_overlay",
        "swap=3412 raw=1234 lo=34\n",
        "",
        &[],
    );
}

/// R14: a whole packed variable is a true alias, visible before return.
#[test]
fn sim_packed_aggregate_formal_ref_aliases() {
    sim_cli::run_case(
        "feature_completion/g1_17",
        "packed_aggregate_formal_ref_rejected",
        "packed ref passed\n",
        "",
        &[],
    );
}

/// R14: the historical rejection fixture now requires private writable inputs.
#[test]
fn sim_packed_aggregate_input_write_is_isolated() {
    sim_cli::run_case(
        "feature_completion/g1_17",
        "packed_aggregate_input_write_rejected",
        "packed input copy passed\n",
        "",
        &[],
    );
}

/// G1-17/G1-15: a fixed packed struct return value is an activation-owned
/// slot; the caller copies the whole result and the input formal is unchanged.
#[test]
fn sim_packed_struct_return() {
    sim_cli::run_case(
        "feature_completion/g1_17",
        "packed_struct_return",
        "swap=3412 widen=13cb src=1234\n",
        "",
        &[],
    );
}
