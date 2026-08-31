//! End-to-end simulator tests for signed arithmetic coercion, assignment
//! context sizing, and the signed 64-bit `INT64_MIN / -1` edge case.
//!
//! Each test runs Surelog compile + elaborate, codegen, CMake model build,
//! and executable simulation, asserting the exact stdout.

use std::process::Command;
use std::sync::Mutex;

use llg::core::compile;
use llg::sim;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Compile and run one top-level `tb` design. The caller holds
/// `SURELOG_LOCK`, so Surelog's process-wide state and the temporary CWD are
/// serialized with the other simulator integration tests.
fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    let dir = std::env::temp_dir().join(format!("llg_sim_arithmetic_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir: {e}"))?;
    let src = dir.join("tb.sv");
    std::fs::write(&src, sv).map_err(|e| format!("write source: {e}"))?;

    let orig_cwd = std::env::current_dir().map_err(|e| format!("current dir: {e}"))?;
    std::env::set_current_dir(&dir).map_err(|e| format!("chdir to temp dir: {e}"))?;
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

fn assert_stdout(tag: &str, sv: &str, expected: &str) {
    let _guard = SURELOG_LOCK.lock().unwrap();
    let stdout = run_sim(sv, tag).expect("simulation should run");
    assert_eq!(stdout, expected);
}

/// The common arithmetic type is signed only when both operands are signed.
/// The 4-bit `4'shF` value is therefore sign-extended for the signed cases,
/// but zero-extended to `8'b0000_1111` for the mixed signed/unsigned cases.
#[test]
fn sim_signed_arithmetic_coercion() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic signed [3:0] s4_neg1;
    logic signed [7:0] s8_pos1;
    logic signed [7:0] s8_two;
    logic [7:0] u8_80;
    logic [7:0] u8_two;
    logic signed [7:0] signed_sum;
    logic signed [7:0] signed_product;
    logic [7:0] mixed_sum;
    logic [7:0] unsigned_quotient;
    logic [7:0] unsigned_remainder;

    initial begin
        s4_neg1 = 4'shF;       // bits 1111, signed value -1
        s8_pos1 = 8'sd1;       // bits 0000_0001, signed value +1
        s8_two = 8'sd2;        // bits 0000_0010, signed value +2
        u8_80 = 8'h80;         // bits 1000_0000, unsigned value 128
        u8_two = 8'd2;          // bits 0000_0010, unsigned value 2

        signed_sum = s4_neg1 + s8_pos1;
        signed_product = s4_neg1 * s8_two;
        mixed_sum = s4_neg1 + u8_80;
        unsigned_quotient = s4_neg1 / u8_two;
        unsigned_remainder = s4_neg1 % u8_two;

        $display("sum=%0d product=%0d mixed=%0d quotient=%0d remainder=%0d",
                 signed_sum, signed_product, mixed_sum,
                 unsigned_quotient, unsigned_remainder);
        $finish;
    end
endmodule
"#;
    assert_stdout(
        "coercion",
        sv,
        "sum=0 product=-2 mixed=143 quotient=7 remainder=1\n",
    );
}

/// A sized arithmetic expression inherits the 16-bit signed assignment
/// context, so `8'sh7F + 8'sd1` is evaluated as 16-bit `128` rather than
/// overflowing as an 8-bit sum.
#[test]
fn sim_signed_assignment_context_width() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic signed [7:0] a;
    logic signed [7:0] b;
    logic signed [15:0] y;

    initial begin
        a = 8'sh7F;             // bits 0111_1111, signed value +127
        b = 8'sd1;              // bits 0000_0001, signed value +1
        y = a + b;              // assignment context evaluates at 16 bits
        $display("y=%0d bits=%h", y, y);
        $finish;
    end
endmodule
"#;
    assert_stdout("assignment_context", sv, "y=128 bits=0080\n");
}

/// Signed division and modulo must handle the two's-complement overflow edge
/// without executing C's undefined `INT64_MIN / -1` operation.
#[test]
fn sim_signed_int64_min_division_and_modulo() {
    if !llg::sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic signed [63:0] min_value;
    logic signed [63:0] negative_one;
    logic signed [63:0] quotient;
    logic signed [63:0] remainder;

    initial begin
        min_value = 64'sh8000_0000_0000_0000;
        negative_one = 64'shFFFF_FFFF_FFFF_FFFF;
        quotient = min_value / negative_one;
        remainder = min_value % negative_one;
        $display("quotient=%h remainder=%0d", quotient, remainder);
        $finish;
    end
endmodule
"#;
    assert_stdout("int64_min", sv, "quotient=8000000000000000 remainder=0\n");
}

#[test]
fn sim_unary_operators_receive_assignment_width() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic [7:0] negated;
    logic [7:0] inverted;
    logic [7:0] nested;

    initial begin
        negated = -4'sb1000;
        inverted = ~4'b0000;
        nested = 8'd1 + ~4'b0000;
        $display("negated=%h inverted=%h nested=%h", negated, inverted, nested);
        $finish;
    end
endmodule
"#;
    assert_stdout(
        "unary_assignment_context",
        sv,
        "negated=08 inverted=ff nested=00\n",
    );
}
