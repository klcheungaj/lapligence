//! End-to-end simulator tests for `real` and `shortreal` values.
//!
//! The positive cases cover procedural assignment, real parameter arithmetic,
//! mixed real/integer conditional typing, shortreal rounding, `$display`'s
//! real format precision, implicit real-to-integer rounding, and real-valued
//! non-blocking assignment timing and wide packed conversion. The rejection
//! cases retain unsupported procedural contexts such as monitors while the
//! supported real-array, continuous-assignment, and function paths are tested
//! below.
//!
//! Each
//! test uses a fresh temp directory and the process-wide mutex serializes
//! compile/codegen runs with the other simulator integration tests.
#[path = "support/sim.rs"]
mod sim_harness;

use std::sync::Mutex;

use llg::core::{compile, db::Db};
use llg::ffi::slang::DiagnosticSeverity;
use llg::sim::{self, opt::OptConfig};

static CWD_LOCK: Mutex<()> = Mutex::new(());

const REAL_ARRAY_FUNCTION_SOURCE: &str = r#"module tb;
    real values [0:1];
    shortreal rounded [0:0];
    real result;

    function real twice(input real value);
        twice = value * 2.0;
    endfunction

    assign result = values[1] + 0.5;

    initial begin
        values[0] = 1.25;
        values[1] = twice(values[0]);
        rounded[0] = values[1] + 0.0000001;
        #1;
        $display("v0=%f v1=%f result=%f rounded=%f", values[0], values[1], result, rounded[0]);
        $finish;
    end
endmodule
"#;

fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    sim_harness::run_sim(sv, "tb", tag)
}

/// Compile and codegen a design without building or running the C model.
/// This preserves the codegen error for a negative case.
fn codegen_result(
    sv: &str,
    tag: &str,
) -> Result<Result<sim::codegen::GeneratedModel, String>, String> {
    sim_harness::with_temp_cwd(tag, |dir| {
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
        let db =
            llg::core::db::Db::from_slang(&out.snapshot).map_err(|error| format!("db: {error}"))?;
        Ok(sim::codegen::generate(&db).map_err(|error| error.to_string()))
    })
}

#[test]
fn sim_real_assignment_and_arithmetic() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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
    let _guard = CWD_LOCK.lock().unwrap();
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
fn sim_real_to_128_bit_packed_conversion() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_type_edges/real_to_wide.sv");
    let source = std::fs::read_to_string(&fixture).expect("read real-to-wide fixture");
    let stdout = run_sim(&source, "wide-conversion").expect("wide conversion should run");
    assert_eq!(stdout, "PASS real_to_wide WIDTH=128\n");
}

#[test]
fn sim_real_unsupported_contexts_are_rejected() {
    let _guard = CWD_LOCK.lock().unwrap();
    let continuous = r#"module tb;
    wire real r;
    assign r = 1.0;
endmodule
"#;
    let diagnostics =
        sim_harness::frontend_diagnostics(continuous, "tb").expect("compile real net");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Error && diagnostic.name == "InvalidNetType"
        }),
        "real net must report InvalidNetType: {diagnostics:?}"
    );

    let invalid_real_expressions = [
        (
            "select",
            r#"module tb;
    real r;
    initial begin
        r = 1.0;
        r[0] = 1'b1;
    end
endmodule
"#,
            "cannot be indexed",
        ),
        (
            "bitwise",
            r#"module tb;
    real r;
    initial begin
        r = 1.0;
        r = r & 1.0;
    end
endmodule
"#,
            "invalid operands",
        ),
        (
            "edge",
            r#"module tb;
    real r;
    initial begin
        @(posedge r);
    end
endmodule
"#,
            "not integral",
        ),
    ];
    for (tag, source, expected) in invalid_real_expressions {
        let diagnostics =
            sim_harness::frontend_diagnostics(source, "tb").expect("compile invalid real form");
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.severity == DiagnosticSeverity::Error
                    && diagnostic.message.to_ascii_lowercase().contains(expected)
            }),
            "{tag} real form must remain a diagnostic: {diagnostics:?}"
        );
    }

    let cases = [
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
    ];

    for (tag, sv, expected) in cases {
        let result = codegen_result(sv, tag).expect("Slang compile should succeed");
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

#[test]
fn sim_real_arrays_and_function_returns_preserve_fractional_values() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    let stdout = run_sim(REAL_ARRAY_FUNCTION_SOURCE, "arrays_function_continuous")
        .expect("real arrays/function/continuous assignment should run");
    assert_eq!(
        stdout,
        "v0=1.250000 v1=2.500000 result=3.000000 rounded=2.500000\n"
    );
}

#[test]
fn sim_real_arrays_function_and_continuous_assignment_match_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let _guard = CWD_LOCK.lock().unwrap();
    sim_harness::with_frontend_temp_cwd("arrays_function_continuous_parity", |dir| {
        let source = dir.join("tb.sv");
        std::fs::write(&source, REAL_ARRAY_FUNCTION_SOURCE)
            .map_err(|error| format!("write source: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let db =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;
        let expected = "v0=1.250000 v1=2.500000 result=3.000000 rounded=2.500000\n";
        for (variant, options) in [
            ("optimized", OptConfig::default()),
            ("unoptimized", OptConfig::none()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&db, &options)
                .map_err(|error| format!("codegen {variant}: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| format!("cmake {variant}: {error}"))?;
            assert_eq!(
                sim_harness::run_executable(&executable)?,
                expected,
                "{variant} output"
            );
        }
        Ok(())
    })
    .expect("real array/function optimizer parity");
}
