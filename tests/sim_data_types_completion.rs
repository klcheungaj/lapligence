//! Independent SystemVerilog datatype completion fixtures derived from the
//! local IEEE 1800-2009 specification. Every positive fixture must execute
//! successfully with optimization disabled and enabled.

#[path = "support/sim.rs"]
mod sim_harness;

use std::path::Path;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

fn run_fixture(file: &str, label: &str) {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_completion")
        .join(file);
    let expected = format!("PASS {label}\n");

    sim_harness::with_frontend_temp_cwd("data-types-completion", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("{file}: compile: {error}"))?;
        let database = Db::from_slang(&compiled.snapshot)
            .map_err(|error| format!("{file}: database: {error}"))?;

        let mut failures = Vec::new();
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let actual = (|| {
                let model = sim::codegen::generate_from_db_with_opts(&database, &options)
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
            Err(format!("{file}:\n{}", failures.join("\n")))
        }
    })
    .expect("datatype completion conformance");
}

fn run_reduction_with_rejection_fixture(file: &str) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_completion")
        .join(file);

    sim_harness::with_frontend_temp_cwd("data-types-completion-rejection", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("{file}: compile: {error}"))?;
        let database = Db::from_slang(&compiled.snapshot)
            .map_err(|error| format!("{file}: database: {error}"))?;

        let mut failures = Vec::new();
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            match sim::codegen::generate_from_db_with_opts(&database, &options) {
                Ok(_) => failures.push(format!(
                    "{variant}: codegen unexpectedly accepted reduction with clause"
                )),
                Err(error) => {
                    let error = error.to_string();
                    if !error.contains(
                        "container method `sum` with a `with` clause in `tb` is not supported",
                    ) {
                        failures.push(format!("{variant}: unexpected diagnostic: {error}"));
                    }
                }
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!("{file}:\n{}", failures.join("\n")))
        }
    })
    .expect("reduction with-clause rejection conformance");
}

#[test]
fn string_atoreal_and_realtoa() {
    run_fixture("string_real_conversion.sv", "string_real_conversion");
}

#[test]
fn string_real_methods_use_wide_conversion_contexts() {
    run_fixture("string_wide_real_contexts.sv", "string_wide_real_contexts");
}

#[test]
fn packed_aggregate_assignment_patterns() {
    run_fixture(
        "packed_struct_assignment_patterns.sv",
        "packed_struct_assignment_patterns",
    );
}

#[test]
fn packed_union_initialization_and_member_writes() {
    run_fixture(
        "packed_union_assignment_patterns.sv",
        "packed_union_initialization_and_member_writes",
    );
}

#[test]
fn unpacked_struct_patterns_and_union_member_writes() {
    run_fixture(
        "unpacked_aggregate_assignment_patterns.sv",
        "unpacked_struct_patterns_and_union_member_writes",
    );
}

#[test]
fn dynamic_array_reductions_preserve_element_width() {
    run_fixture("dynamic_array_reductions.sv", "dynamic_array_reductions");
}

#[test]
fn queue_reductions_preserve_signed_element_type() {
    run_fixture("queue_reductions.sv", "queue_reductions");
}

#[test]
fn associative_array_reductions_include_wide_high_bits() {
    run_fixture(
        "associative_array_reductions.sv",
        "associative_array_reductions",
    );
}

#[test]
fn reduction_with_is_explicitly_unsupported() {
    run_reduction_with_rejection_fixture("reduction_with_unsupported.sv");
}
