//! End-to-end operator conformance tests.
//!
//! Each case runs the standard Surelog → codegen → CMake → executable path
//! and checks the complete stdout trace.

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use llg::core::compile;
use llg::core::db::Db;
use llg::sim;
use llg::sim::opt::OptConfig;

static SURELOG_LOCK: Mutex<()> = Mutex::new(());

/// Run an action from a fresh temporary CWD, then restore the caller's CWD
/// before removing the temporary design tree.
fn with_temp_design<T>(
    sv: &str,
    tag: &str,
    action: impl FnOnce(&Path, &Path) -> Result<T, String>,
) -> Result<T, String> {
    let dir = std::env::temp_dir().join(format!("llg_sim_operator_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("create temp dir: {e}"))?;
    let src = dir.join("tb.sv");
    std::fs::write(&src, sv).map_err(|e| format!("write source: {e}"))?;

    let orig_cwd = std::env::current_dir().map_err(|e| format!("current dir: {e}"))?;
    if let Err(e) = std::env::set_current_dir(&dir) {
        let _ = std::fs::remove_dir_all(&dir);
        return Err(format!("chdir to temp dir: {e}"));
    }
    let result = action(&dir, &src);
    let restore = std::env::set_current_dir(&orig_cwd);
    let _ = std::fs::remove_dir_all(&dir);

    match (result, restore) {
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(format!("restore cwd: {error}")),
        (Ok(value), Ok(())) => Ok(value),
    }
}

fn run_executable(exe: &Path, variant: &str) -> Result<String, String> {
    let output = Command::new(exe)
        .output()
        .map_err(|e| format!("run {variant}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "simulation {variant} exited with {:?}, stderr: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn run_sim(sv: &str, tag: &str) -> Result<String, String> {
    with_temp_design(sv, tag, |dir, src| {
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
        let exe = sim::build::build_model_cmake(dir, &[("model.c", generated.model_c.as_str())])
            .map_err(|e| format!("cmake: {e}"))?;
        run_executable(&exe, "default")
    })
}

fn codegen_error(sv: &str, tag: &str) -> Result<String, String> {
    with_temp_design(sv, tag, |_dir, src| {
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
        match sim::codegen::generate(design) {
            Ok(_) => Err("codegen unexpectedly succeeded".to_string()),
            Err(error) => Ok(error),
        }
    })
}

fn run_optimized_variants(sv: &str, tag: &str) -> Result<(String, String), String> {
    with_temp_design(sv, tag, |dir, src| {
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
        let db = Db::build(design).map_err(|e| format!("db: {e}"))?;
        let optimized = sim::codegen::generate_from_db_with_opts(&db, &OptConfig::default())
            .map_err(|e| format!("codegen(opt-on): {e}"))?;
        let unoptimized = sim::codegen::generate_from_db_with_opts(&db, &OptConfig::none())
            .map_err(|e| format!("codegen(opt-off): {e}"))?;

        let on_dir = dir.join("opt_on");
        let on_exe =
            sim::build::build_model_cmake(&on_dir, &[("model.c", optimized.model_c.as_str())])
                .map_err(|e| format!("cmake(opt-on): {e}"))?;
        let on = run_executable(&on_exe, "opt-on")?;

        let off_dir = dir.join("opt_off");
        let off_exe =
            sim::build::build_model_cmake(&off_dir, &[("model.c", unoptimized.model_c.as_str())])
                .map_err(|e| format!("cmake(opt-off): {e}"))?;
        let off = run_executable(&off_exe, "opt-off")?;

        Ok((on, off))
    })
}

fn assert_stdout(tag: &str, sv: &str, expected: &str) {
    let _guard = SURELOG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let stdout = run_sim(sv, tag).expect("simulation should run");
    assert_eq!(stdout, expected);
}

#[test]
fn sim_statement_increment_and_decrement() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ps
module tb;
    integer i;
    integer sum;
    logic [3:0] nibble;

    initial begin
        sum = 0;
        for (i = 0; i < 4; i++)
            sum = sum + i;

        ++sum;
        sum--;
        nibble = 4'h0;
        nibble++;
        --nibble;

        $display("i=%0d sum=%0d nibble=%0d", i, sum, nibble);
        $finish;
    end
endmodule
"#;
    assert_stdout("inc_dec_statement", sv, "i=4 sum=6 nibble=0\n");
}

#[test]
fn sim_scalar_enum_variables_use_their_packed_base_type() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ps
module tb;
    typedef enum logic [2:0] {
        IDLE = 3'd0,
        LOAD = 3'd3,
        RUN  = 3'd5
    } state_t;
    state_t state;

    initial begin
        state = LOAD;
        if (state == LOAD)
            state = RUN;
        $display("state=%0d bits=%0d", state, $bits(state));
        $finish;
    end
endmodule
"#;
    assert_stdout("scalar_enum", sv, "state=5 bits=3\n");
}

#[test]
fn sim_whole_variable_compound_assignments() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"`timescale 1ns/1ps
module tb;
    integer value;
    logic signed [3:0] signed_value;

    initial begin
        value = 10;
        value += 5;
        value -= 3;
        value *= 2;
        value /= 5;
        value %= 3;
        value <<= 3;
        value >>= 1;
        value ^= 3;
        value |= 8;
        value &= 6;

        signed_value = -4;
        signed_value >>>= 1;
        signed_value <<<= 1;

        $display("value=%0d signed=%0d", value, signed_value);
        $finish;
    end
endmodule
"#;
    assert_stdout("compound_assign", sv, "value=6 signed=-4\n");
}

#[test]
fn sim_increment_and_compound_assignment_reject_unsupported_positions() {
    let _guard = SURELOG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let select_error = codegen_error(
        r#"module tb;
    logic [3:0] value;
    initial begin
        value = 0;
        value[1:0] += 1;
        $finish;
    end
endmodule
"#,
        "compound_select_reject",
    )
    .expect("compound select must be rejected");
    assert!(
        select_error.contains("compound assignment to a select or array element"),
        "unexpected select error: {select_error}"
    );

    let increment_select_error = codegen_error(
        r#"module tb;
    logic [3:0] value;
    initial begin
        value = 0;
        value[0]++;
        $finish;
    end
endmodule
"#,
        "inc_select_reject",
    )
    .expect("increment select must be rejected");
    assert!(
        increment_select_error.contains("increment/decrement of a select or array element"),
        "unexpected increment-select error: {increment_select_error}"
    );

    let expression_error = codegen_error(
        r#"module tb;
    integer value;
    integer result;
    initial begin
        value = 0;
        result = value++;
        $finish;
    end
endmodule
"#,
        "inc_expression_reject",
    )
    .expect("expression-valued increment must be rejected");
    assert!(
        expression_error.contains("unsupported operation op type"),
        "unexpected increment-expression error: {expression_error}"
    );
}

#[test]
fn sim_mixed_signed_unsigned_bitwise_operators() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic signed [3:0] signed_nibble;
    logic [7:0] and_value;
    logic [7:0] and_zero;
    logic [7:0] or_value;
    logic [7:0] xor_value;
    logic [7:0] xnor_value;
    logic [7:0] not_value;

    initial begin
        signed_nibble = 4'shf;
        and_value = signed_nibble & 8'h0f;
        and_zero = signed_nibble & 8'hf0;
        or_value = signed_nibble | 8'h00;
        xor_value = signed_nibble ^ 8'h00;
        xnor_value = 4'sh0 ~^ 8'h0f;
        not_value = ~8'h0f;
        $display("and=%h and_zero=%h or=%h xor=%h xnor=%h not=%h",
                 and_value, and_zero, or_value, xor_value, xnor_value, not_value);
        $finish;
    end
endmodule
"#;
    assert_stdout(
        "bitwise",
        sv,
        "and=0f and_zero=00 or=0f xor=0f xnor=f0 not=f0\n",
    );
}

#[test]
fn sim_equality_relational_and_case_equality_coercion() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic mixed_eq;
    logic mixed_ne;
    logic mixed_lt;
    logic mixed_ge;
    logic signed_eq;
    logic signed_ne;
    logic signed_lt;
    logic signed_ge;
    logic case_mixed;
    logic case_signed;

    initial begin
        mixed_eq = 4'shf == 8'hff;
        mixed_ne = 4'shf != 8'hff;
        mixed_lt = 4'shf < 8'hff;
        mixed_ge = 4'shf >= 8'hff;
        signed_eq = 4'shf == 8'shff;
        signed_ne = 4'shf != 8'shff;
        signed_lt = 4'shf < 8'shff;
        signed_ge = 4'shf >= 8'shff;
        case_mixed = 4'shf === 8'hff;
        case_signed = 4'shf === 8'shff;
        $display("mixed=%b%b%b%b signed=%b%b%b%b case=%b%b",
                 mixed_eq, mixed_ne, mixed_lt, mixed_ge,
                 signed_eq, signed_ne, signed_lt, signed_ge,
                 case_mixed, case_signed);
        $finish;
    end
endmodule
"#;
    assert_stdout("comparison", sv, "mixed=0110 signed=1001 case=01\n");
}

#[test]
fn sim_conditional_known_coercion_and_unknown_merge() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic [7:0] known_select;
    logic [7:0] unknown_select;

    initial begin
        known_select = 1'b1 ? 4'shf : 8'h00;
        unknown_select = 1'bx ? 8'b0000_0000 : 8'b0000_1111;
        $display("known=%h unknown=%b", known_select, unknown_select);
        $finish;
    end
endmodule
"#;
    assert_stdout("conditional", sv, "known=0f unknown=0000xxxx\n");
}

#[test]
fn sim_shift_width_and_signed_unknown_patterns() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic [3:0] unsigned_ashr;
    logic signed [3:0] signed_ashr;
    logic [3:0] left_x;
    logic signed [3:0] signed_x;
    logic signed [3:0] signed_z;
    logic signed [7:0] widened_x;
    logic signed [7:0] widened_z;

    initial begin
        unsigned_ashr = 4'b1000 >>> 1;
        signed_ashr = 4'sb1000 >>> 1;
        left_x = 4'b1x01 << 1;
        signed_x = 4'sbx101;
        signed_z = 4'sbz101;
        widened_x = signed_x;
        widened_z = signed_z;
        $display("ushr=%b sshr=%b shl=%b xwide=%b zwide=%b",
                 unsigned_ashr, signed_ashr, left_x, widened_x, widened_z);
        $finish;
    end
endmodule
"#;
    assert_stdout(
        "shift_and_widen",
        sv,
        "ushr=0100 sshr=1100 shl=x010 xwide=xxxxx101 zwide=zzzzz101\n",
    );
}

#[test]
fn sim_optimizer_preserves_double_negation_of_z() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let sv = r#"module tb;
    logic [7:0] z_value;
    logic [7:0] double_negated;

    initial begin
        z_value = 8'bzzzz_zzzz;
        double_negated = ~~z_value;
        $display("double=%b", double_negated);
        $finish;
    end
endmodule
"#;

    let _guard = SURELOG_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (optimized, unoptimized) =
        run_optimized_variants(sv, "double_z").expect("both optimizer variants should run");
    assert_eq!(optimized, "double=xxxxxxxx\n");
    assert_eq!(
        unoptimized, optimized,
        "optimizer changed double-negation semantics"
    );
}
