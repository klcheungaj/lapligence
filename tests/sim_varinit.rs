//! End-to-end simulator tests for scalar VARIABLE declaration initializers
//! (`logic l = 1'b0;`, `int x = 5;`, captured by `core::db` in
//! `Db::vars_init`): Slang compile → codegen → CMake build → run.
//!
//! The `reg`/`wire` initializer forms are covered in `tests/sim_geninit.rs`.
//!
//! These tests temporarily change the process working directory, so each
//! test runs with the CWD pointed at a fresh temp dir (serialized through a
//! mutex, to avoid process-wide CWD races).

#[path = "support/sim.rs"]
mod sim_harness;

/// Compile and run one `tb` design through the shared simulator harness.
fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    sim_harness::run_sim(sv, "tb", tag)
}

/// Variable declaration initializers (`logic l = 1'b0;`, `logic [7:0] v =
/// 8'ha5;`, `int x = 5;`, previously left X) must be
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

/// A variable declaration initializer may read a runtime signal in
/// SystemVerilog. It evaluates before ordinary processes, so a later t=0
/// write does not retroactively change the initialized value.
#[test]
fn sim_var_init_nonconst_runs_before_processes() {
    let sv = r#"module tb;
    reg a;
    logic z = a;
    initial begin
        a = 1'b1;
        $display("a=%b z=%b", a, z);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "varnonconst").expect("simulation should run");
    assert_eq!(stdout, "a=1 z=x\n");
}
