//! End-to-end simulator tests for `real` and `shortreal` values.
//!
//! The positive cases cover procedural assignment, real parameter arithmetic,
//! mixed real/integer conditional typing, shortreal rounding, `$display`'s
//! real format precision, implicit real-to-integer rounding, and real-valued
//! non-blocking assignment timing.  The rejection cases pin the documented v1
//! boundary: real values are procedural-only and are not supported in
//! combinational processes, waits, monitors, arrays, ports/links, functions,
//! or real-to-packed conversions wider than 64 bits.
//!
//! Surelog writes `slpp_all/` into the process working directory, so each
//! test uses a fresh temp directory and the process-wide mutex serializes
//! compile/codegen runs with the other simulator integration tests.

use std::process::Command;
use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Compile, codegen, compile the generated model, and run it; return stdout.
fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("llg_sim_real_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir: {e}"))?;
    let src = dir.join("tb.sv");
    std::fs::write(&src, sv).map_err(|e| format!("write source: {e}"))?;

    let orig_cwd = std::env::current_dir().map_err(|e| format!("current dir: {e}"))?;
    std::env::set_current_dir(&dir).map_err(|e| format!("chdir: {e}"))?;
    let result = (|| -> Result<String, String> {
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
        let generated = sim::codegen::generate(design).map_err(|e| format!("codegen: {e}"))?;
        let exe = sim::build::build_model_cmake(&dir, &[("model.c", generated.model_c.as_str())])
            .map_err(|e| format!("cmake: {e}"))?;
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
    })();

    std::env::set_current_dir(&orig_cwd).map_err(|e| format!("restore cwd: {e}"))?;
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Compile and codegen a design without building or running the C model.
/// This preserves the codegen error for a negative case.
fn codegen_result(
    sv: &str,
    tag: &str,
) -> Result<Result<sim::codegen::GeneratedModel, String>, String> {
    let dir = std::env::temp_dir().join(format!("llg_sim_real_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir: {e}"))?;
    let src = dir.join("tb.sv");
    std::fs::write(&src, sv).map_err(|e| format!("write source: {e}"))?;

    let orig_cwd = std::env::current_dir().map_err(|e| format!("current dir: {e}"))?;
    std::env::set_current_dir(&dir).map_err(|e| format!("chdir: {e}"))?;
    let result = (|| -> Result<Result<sim::codegen::GeneratedModel, String>, String> {
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
        Ok(sim::codegen::generate(design))
    })();

    std::env::set_current_dir(&orig_cwd).map_err(|e| format!("restore cwd: {e}"))?;
    let _ = std::fs::remove_dir_all(&dir);
    result
}

#[test]
fn sim_real_assignment_and_arithmetic() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    real r;
    shortreal sr;
    real sum;
    shortreal half;

    initial begin
        r = 1.25;
        sr = r;
        sum = r + sr * 2.0;
        half = sum / 2.0;
        r = half + 0.5;
        $display("r=%f sr=%f sum=%f half=%f", r, sr, sum, half);
        $finish;
    end
endmodule
"#;

    // r=1.25, sr=1.25, sum=3.75, half=1.875, and the final r=2.375.
    let stdout = run_sim(sv, "assign").expect("real simulation should run");
    assert_eq!(
        stdout,
        "r=2.375000 sr=1.250000 sum=3.750000 half=1.875000\n"
    );
}

#[test]
fn sim_real_parameter_constant_arithmetic() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    parameter real BASE = 1.25;
    parameter real SCALE = 2.0;
    localparam real TOTAL = BASE * SCALE + 0.5;
    localparam integer TO_INT = 2.6;
    localparam real FROM_INT = 7;
    localparam shortreal SHORT = 16777217.0;
    localparam logic [127:0] WIDE = 128'h00000000000000010000000000000000;
    localparam real WIDE_REAL = WIDE;
    localparam logic [3:0] PARTIAL = 4'b1x01;
    localparam real PARTIAL_REAL = PARTIAL;
    real observed;

    initial begin
        observed = TOTAL;
        $display("total=%.2f int=%0d real=%.2f short=%.0f wide=%.0f partial=%.0f",
                 observed, TO_INT, FROM_INT, SHORT, WIDE_REAL, PARTIAL_REAL);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "param_arithmetic").expect("real parameter simulation should run");
    assert_eq!(
        stdout,
        "total=3.00 int=3 real=7.00 short=16777216 wide=18446744073709551616 partial=9\n"
    );
}

#[test]
fn sim_real_integer_conditional_has_real_result_type() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    logic choose_real;

    initial begin
        choose_real = 1'b1;
        $display("real-branch=%.2f", choose_real ? 1.25 : 2);
        choose_real = 1'b0;
        $display("integer-branch=%.2f", choose_real ? 1.25 : 2);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "conditional").expect("mixed conditional simulation should run");
    assert_eq!(stdout, "real-branch=1.25\ninteger-branch=2.00\n");
}

#[test]
fn sim_real_control_flow_uses_scalar_truth_conversion() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    real condition;
    integer while_count;
    integer repeat_count;

    initial begin
        condition = 0.0;
        if (condition)
            $display("zero-branch");
        else
            $display("zero-false");

        condition = 2.5;
        if (condition)
            $display("nonzero-true");
        else
            $display("nonzero-branch");

        while_count = 0;
        condition = 2.0;
        while (condition) begin
            while_count = while_count + 1;
            condition = condition - 1.0;
        end

        repeat_count = 0;
        repeat (2) begin
            repeat_count = repeat_count + 1;
        end
        $display("while-count=%0d condition=%.2f repeat-count=%0d",
                 while_count, condition, repeat_count);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "control_flow").expect("real control-flow simulation should run");
    assert_eq!(
        stdout,
        "zero-false\nnonzero-true\nwhile-count=2 condition=0.00 repeat-count=2\n"
    );
}

#[test]
fn sim_shortreal_rounds_to_single_precision() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    real full_precision;
    shortreal rounded;
    real cast_rounded;

    initial begin
        full_precision = 16777217.0;
        rounded = full_precision;
        cast_rounded = shortreal'(16777217.0);
        $display("real=%.2f shortreal=%.2f cast=%.2f",
                 full_precision, rounded, cast_rounded);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "shortreal_rounding").expect("shortreal simulation should run");
    assert_eq!(
        stdout,
        "real=16777217.00 shortreal=16777216.00 cast=16777216.00\n"
    );
}

#[test]
fn sim_real_to_integer_assignment_rounds_to_nearest() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    real source;
    integer rounded;
    logic [7:0] wrapped;
    logic signed [7:0] truncated;

    initial begin
        source = 2.6;
        rounded = source;
        source = -2.5;
        wrapped = source;
        source = 130.0;
        truncated = source;
        $display("rounded=%0d wrapped=%0d truncated=%0d",
                 rounded, wrapped, truncated);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "real_to_integer_rounding")
        .expect("real-to-integer conversion simulation should run");
    assert_eq!(stdout, "rounded=3 wrapped=253 truncated=-126\n");
}

#[test]
fn sim_real_display_formats() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    real value;

    initial begin
        value = 12.5;
        $display("f=%f e=%e g=%g", value, value, value);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "display").expect("real display simulation should run");
    assert_eq!(stdout, "f=12.500000 e=1.250000e+01 g=12.5\n");
}

#[test]
fn sim_real_display_precision_rounds_fractional_digits() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    real value;

    initial begin
        value = 12.3456;
        $display("value=%.2f", value);
        $finish;
    end
endmodule
"#;

    let stdout = run_sim(sv, "display_precision").expect("real precision simulation should run");
    assert_eq!(stdout, "value=12.35\n");
}

#[test]
fn sim_real_nonblocking_assignment_commits_after_inactive_region() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let sv = r#"module tb;
    real r;
    shortreal sr;

    initial begin
        r = 1.25;
        sr = 1.25;
        r <= 2.5;
        sr <= 2.5;
        $display("active r=%f sr=%f", r, sr);
        #0 $display("inactive r=%f sr=%f", r, sr);
        #1 $display("committed r=%f sr=%f", r, sr);
        $finish;
    end
endmodule
"#;

    // The active and inactive-region reads precede the NBA commit.  The
    // following time step observes both real-valued NBA writes.
    let stdout = run_sim(sv, "nba").expect("real NBA simulation should run");
    assert_eq!(
        stdout,
        "active r=1.250000 sr=1.250000\n".to_owned()
            + "inactive r=1.250000 sr=1.250000\n"
            + "committed r=2.500000 sr=2.500000\n"
    );
}

#[test]
fn sim_real_unsupported_contexts_are_rejected() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = SURELOG_LOCK.lock().unwrap();
    let cases = [
        (
            "comb",
            r#"module tb;
    real r;
    always_comb r = 1.0;
endmodule
"#,
            "comb",
        ),
        (
            "continuous",
            r#"module tb;
    wire real r;
    assign r = 1.0;
endmodule
"#,
            "continuous",
        ),
        (
            "repeat",
            r#"module tb;
    real count;
    initial begin
        count = 2.0;
        repeat (count) count = count - 1.0;
    end
endmodule
"#,
            "repeat",
        ),
        (
            "wait",
            r#"module tb;
    real r;
    initial wait (r);
endmodule
"#,
            "wait",
        ),
        (
            "monitor",
            r#"module tb;
    real r;
    initial begin
        r = 1.0;
        $monitor("r=%f", r);
    end
endmodule
"#,
            "monitor",
        ),
        (
            "array",
            r#"module tb;
    real values [0:1];
    initial values[0] = 1.0;
endmodule
"#,
            "array",
        ),
        (
            "port",
            r#"module child(input real in_value);
endmodule

module tb;
    real value;
    child u_child(.in_value(value));
endmodule
"#,
            "port",
        ),
        (
            "wide-conversion",
            r#"module tb;
    real value;
    logic [127:0] wide;

    initial begin
        value = 3.5;
        wide = value;
    end
endmodule
"#,
            "64",
        ),
        (
            "function",
            r#"module tb;
    real r;

    function real twice(input logic value);
        twice = value;
    endfunction

    initial r = twice(1'b1);
endmodule
"#,
            "function",
        ),
    ];

    for (tag, sv, expected) in cases {
        let result = codegen_result(sv, tag).expect("Surelog compile should succeed");
        let err = match result {
            Ok(_) => panic!("codegen should reject the unsupported real context"),
            Err(err) => err,
        };
        assert!(
            err.to_ascii_lowercase().contains("real")
                && err.to_ascii_lowercase().contains(expected),
            "{tag} rejection was not clear enough: {err}"
        );
    }
}
