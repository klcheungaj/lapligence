//! End-to-end simulator tests for scalar VARIABLE declaration initializers
//! (`logic l = 1'b0;`, `int x = 5;` — the form whose init lives on the var's
//! `vpiExpr`, captured by `core::db` in `Db::vars_init`): Surelog compile →
//! codegen → CMake build → run.
//!
//! The `reg`/`wire` initializer forms (which surface as `vpiNetDeclAssign`
//! continuous assignments) are covered in `tests/sim_geninit.rs`.
//!
//! Surelog writes `slpp_all/` into the process working directory, so each
//! test runs with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, like the other Surelog integration tests).

use llg::core::compile;
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

/// Compile and run one `tb` design through the shared simulator harness.
fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    sim_harness::run_sim(sv, "tb", tag)
}

/// Variable declaration initializers (`logic l = 1'b0;`, `logic [7:0] v =
/// 8'ha5;`, `int x = 5;` — the var-`vpiExpr` form, previously left X) must be
/// applied in `main()` before any process runs, so a t=0 `$display` sees the
/// declared values.
///
/// Hand-simulation:
///
///   t=0   main() fills the declaration initializers (l=0, v=8'ha5, x=5)
///        before spawning any process.  Spawn: initial only.
///        initial: $display("l=0 v=a5 x=5"); $finish.
///
/// Expected stdout (exactly):
///   l=0 v=a5 x=5
#[test]
fn sim_var_inits_applied_before_processes() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic l = 1'b0;
    logic [7:0] v = 8'ha5;
    int x = 5;
    initial begin
        $display("l=%b v=%h x=%0d", l, v, x);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "varinit").expect("simulation should run");
    assert_eq!(stdout, "l=0 v=a5 x=5\n");
}

/// A process writing an initialized variable at t=0 must override the
/// declaration initializer (the fill lands in `main()` before any process
/// spawns, so the blocking writes win).
///
/// Hand-simulation:
///
///   t=0   main() fills l=0, v=8'ha5, x=5 before spawning.  Spawn: initial.
///        initial: $display("before: l=0 v=a5 x=5"); blocking writes
///        l=1, v=8'hff, x=42; $display("after: l=1 v=ff x=42"); $finish.
///
/// Expected stdout (exactly):
///   before: l=0 v=a5 x=5
///   after: l=1 v=ff x=42
#[test]
fn sim_var_init_overridden_by_process_write() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic l = 1'b0;
    logic [7:0] v = 8'ha5;
    int x = 5;
    initial begin
        $display("before: l=%b v=%h x=%0d", l, v, x);
        l = 1'b1;
        v = 8'hff;
        x = 42;
        $display("after: l=%b v=%h x=%0d", l, v, x);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "varovr").expect("simulation should run");
    assert_eq!(stdout, "before: l=0 v=a5 x=5\nafter: l=1 v=ff x=42\n");
}

/// A variable initializer referencing a localparam (`int y = P + 1;` with
/// `localparam int P = 3;`) must fold to the parameter's resolved value:
/// the RHS is folded with `eval_bits`, which resolves the param reference
/// through `param_vals` after the instance's parameters are collected.
///
/// Hand-simulation:
///
///   t=0   main() fills y = 3 + 1 = 4 before spawning.  Spawn: initial.
///        initial: $display("y=4"); $finish.
///
/// Expected stdout (exactly):
///   y=4
#[test]
fn sim_var_init_param_expr() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    localparam int P = 3;
    int y = P + 1;
    initial begin
        $display("y=%0d", y);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "varparam").expect("simulation should run");
    assert_eq!(stdout, "y=4\n");
}

/// A based literal's source signedness must survive the scalar variable
/// initializer path: 4'shf sign-extends to 8'hff, while unsigned 4'hf
/// zero-extends to 8'h0f.
#[test]
fn sim_var_init_based_literal_signedness() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic signed [7:0] signed_value = 4'shf;
    logic [7:0] unsigned_value = 4'hf;
    initial begin
        $display("signed=%h unsigned=%h", signed_value, unsigned_value);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "varsigned").expect("simulation should run");
    assert_eq!(stdout, "signed=ff unsigned=0f\n");
}

/// A variable declaration initializer whose RHS is not a constant expression
/// (it references a signal) must be rejected with the variable-initializer
/// error, not silently mis-emitted (v1 is constant-only).
#[test]
fn sim_var_init_nonconst_rejected() {
    let sv = r#"module tb;
    reg a;
    logic z = a;
    initial $finish;
endmodule
"#;

    let result = sim_harness::with_surelog_temp_cwd("varnc", |dir| {
        let source = dir.join("var_nonconst.sv");
        std::fs::write(&source, sv).map_err(|error| format!("write source: {error}"))?;
        let out = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        sim::codegen::generate(design)
            .map(|_| ())
            .map_err(|error| error.to_string())
    });

    let err = result.expect_err("codegen must reject non-constant variable initializers");
    assert!(
        err.contains("variable initializer is not a constant expression"),
        "unexpected error: {err}"
    );
}
