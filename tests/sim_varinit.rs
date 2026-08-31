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

use std::process::Command;
use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Compile `sv`, codegen the model, build the simulator executable and run
/// it, returning the exact stdout.  The caller must hold `SURELOG_LOCK` and
/// have the CWD set to the temp dir.
fn run_sim(dir: &std::path::Path, file: &str, sv: &str) -> Result<String, String> {
    let src = dir.join(file);
    std::fs::write(&src, sv).map_err(|e| format!("write source: {e}"))?;
    // 1. Surelog compile + elaborate.
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

    // 2. Codegen.
    let gen = sim::codegen::generate(design).map_err(|e| format!("codegen: {e}"))?;

    // 3. Build model + runtime + libaco with CMake.
    let exe = sim::build::build_model_cmake(dir, &[("model.c", gen.model_c.as_str())])
        .map_err(|e| format!("cmake: {e}"))?;

    // 4. Run.
    let output = Command::new(&exe)
        .output()
        .map_err(|e| format!("run: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "sim exited with {:?}, stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
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
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_sim_varinit_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
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

    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let result = run_sim(&dir, "var_init.sv", sv);
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);

    let stdout = result.expect("simulation should run");
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
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_sim_varovr_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
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

    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let result = run_sim(&dir, "var_override.sv", sv);
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);

    let stdout = result.expect("simulation should run");
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
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_sim_varparam_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let sv = r#"module tb;
    localparam int P = 3;
    int y = P + 1;
    initial begin
        $display("y=%0d", y);
        $finish;
    end
endmodule
"#;

    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let result = run_sim(&dir, "var_param.sv", sv);
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);

    let stdout = result.expect("simulation should run");
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
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_sim_varsigned_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let sv = r#"module tb;
    logic signed [7:0] signed_value = 4'shf;
    logic [7:0] unsigned_value = 4'hf;
    initial begin
        $display("signed=%h unsigned=%h", signed_value, unsigned_value);
        $finish;
    end
endmodule
"#;

    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    let result = run_sim(&dir, "var_signed.sv", sv);
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);

    let stdout = result.expect("simulation should run");
    assert_eq!(stdout, "signed=ff unsigned=0f\n");
}

/// A variable declaration initializer whose RHS is not a constant expression
/// (it references a signal) must be rejected with the variable-initializer
/// error, not silently mis-emitted (v1 is constant-only).
#[test]
fn sim_var_init_nonconst_rejected() {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let dir = std::env::temp_dir().join(format!("llg_sim_varnc_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    let sv = r#"module tb;
    reg a;
    logic z = a;
    initial $finish;
endmodule
"#;

    let orig_cwd = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(&dir).expect("chdir to temp dir");
    std::fs::write(dir.join("var_nonconst.sv"), sv).expect("write source");
    let result = (|| -> Result<(), String> {
        let out = compile::compile(&compile::CompileOpts {
            files: vec![dir.join("var_nonconst.sv").to_string_lossy().into_owned()],
            top: Some("tb".to_string()),
            ..Default::default()
        })
        .map_err(|e| format!("compile: {e}"))?;
        if !out.ok() {
            return Err(format!("compile diagnostics: {:?}", out.diagnostics));
        }
        let design = out.uhdm_design().ok_or("no UHDM design")?;
        sim::codegen::generate(design).map(|_| ())
    })();
    std::env::set_current_dir(&orig_cwd).expect("restore cwd");
    let _ = std::fs::remove_dir_all(&dir);

    let err = result.expect_err("codegen must reject non-constant variable initializers");
    assert!(
        err.contains("variable initializer is not a constant expression"),
        "unexpected error: {err}"
    );
}
