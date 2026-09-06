//! Independent end-to-end datatype conformance cases. HDL oracles are
//! checked-in and reference-checked without using llg's value implementation.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};
use std::path::Path;

const EXCLUSIVE_PACKED_WIDTH_LIMIT: usize = 1 << 20;

fn fixture_path(file: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_extended")
        .join(file)
}

fn compile_fixture(file: &str, width: usize, source: &Path) -> Result<Db, String> {
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        param_overrides: vec![format!("-PWIDTH={width}")],
        ..Default::default()
    })
    .map_err(|error| format!("{file}, width {width}: compile: {error}"))?;
    Db::build_with_source_files(
        compiled.uhdm_design().ok_or("no UHDM design")?,
        &compiled.frontend_source_files(),
    )
    .map_err(|error| error.to_string())
}

fn run_fixture(file: &str, label: &str, width: usize) {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = fixture_path(file);
    let expected = format!("PASS {label} WIDTH={width}\n");
    sim_harness::with_surelog_temp_cwd("data-types-extended", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let db = compile_fixture(file, width, &source)?;
        let mut failures = Vec::new();
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let actual = (|| {
                let model = sim::codegen::generate_from_db_with_opts(&db, &options)
                    .map_err(|error| format!("lowering: {error}"))?;
                let executable = sim::build::build_model_cmake(
                    &dir.join(variant),
                    &[("model.c", model.model_c.as_str())],
                )
                .map_err(|error| format!("C model build: {error}"))?;
                sim_harness::run_executable(&executable)
            })();
            match actual {
                Ok(actual) if actual == expected => {}
                Ok(actual) => {
                    failures.push(format!("{variant}: expected {expected:?}, got {actual:?}"))
                }
                Err(error) => failures.push(format!("{variant}: {error}")),
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!("{file}, width {width}:\n{}", failures.join("\n")))
        }
    })
    .expect("extended datatype end-to-end conformance");
}

fn reject_fixture_at_width(file: &str, width: usize, rejected_width: usize) {
    let fixture = fixture_path(file);
    sim_harness::with_surelog_temp_cwd("data-types-width-limit", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let db = compile_fixture(file, width, &source)?;
        let mut failures = Vec::new();
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            match sim::codegen::generate_from_db_with_opts(&db, &options) {
                Ok(_) => failures.push(format!(
                    "{variant}: accepted forbidden packed width {rejected_width}"
                )),
                Err(error) => {
                    let message = error.to_string();
                    let normalized = message.to_ascii_lowercase();
                    if !message.contains(&rejected_width.to_string())
                        || !(normalized.contains("limit") || normalized.contains("maximum"))
                    {
                        failures.push(format!(
                            "{variant}: rejection did not identify width {rejected_width} and its limit: {message}"
                        ));
                    }
                }
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("\n"))
        }
    })
    .expect("packed width limit rejection");
}

macro_rules! datatype_case {
    ($name:ident, $file:literal, $label:literal, $width:expr) => {
        #[test]
        fn $name() {
            run_fixture($file, $label, $width);
        }
    };
}

datatype_case!(
    signed_div_mod_2048_bits,
    "wide_signed_div_mod.sv",
    "wide_signed_div_mod",
    2048
);
datatype_case!(
    signed_div_mod_4096_bits,
    "wide_signed_div_mod.sv",
    "wide_signed_div_mod",
    4096
);
datatype_case!(power_edges_2048_bits, "wide_power.sv", "wide_power", 2048);
datatype_case!(power_edges_4096_bits, "wide_power.sv", "wide_power", 4096);
datatype_case!(
    known_mismatch_equality_2048_bits,
    "equality_known_mismatch.sv",
    "equality_known_mismatch",
    2048
);
datatype_case!(
    four_to_two_state_2048_bits,
    "two_state_conversion.sv",
    "two_state_conversion",
    2048
);
datatype_case!(
    mixed_width_and_signedness_2048_bits,
    "mixed_width_signed.sv",
    "mixed_width_signed",
    2048
);
datatype_case!(
    packed_aggregates_2048_bits,
    "packed_aggregates.sv",
    "packed_aggregates",
    2048
);
datatype_case!(
    shifts_concat_conditional_4096_bits,
    "shifts_concat_conditional.sv",
    "shifts_concat_conditional",
    4096
);
datatype_case!(
    shifts_concat_conditional_65536_bits,
    "shifts_concat_conditional.sv",
    "shifts_concat_conditional",
    65536
);
datatype_case!(
    resolved_nets_2048_bits,
    "resolved_nets.v",
    "resolved_nets",
    2048
);
datatype_case!(
    scalable_arithmetic_2048_bits,
    "scalable_arithmetic.sv",
    "scalable_arithmetic",
    2048
);
datatype_case!(
    scalable_arithmetic_65536_bits,
    "scalable_arithmetic.sv",
    "scalable_arithmetic",
    65536
);
datatype_case!(
    maximum_minus_one_width_is_admitted,
    "max_width_probe.sv",
    "max_width_probe",
    EXCLUSIVE_PACKED_WIDTH_LIMIT - 1
);

#[test]
fn exact_exclusive_width_limit_is_rejected() {
    reject_fixture_at_width(
        "max_width_probe.sv",
        EXCLUSIVE_PACKED_WIDTH_LIMIT,
        EXCLUSIVE_PACKED_WIDTH_LIMIT,
    );
}

#[test]
fn intermediate_expression_at_exclusive_width_limit_is_rejected() {
    reject_fixture_at_width(
        "max_width_intermediate.sv",
        EXCLUSIVE_PACKED_WIDTH_LIMIT / 2,
        EXCLUSIVE_PACKED_WIDTH_LIMIT,
    );
}
