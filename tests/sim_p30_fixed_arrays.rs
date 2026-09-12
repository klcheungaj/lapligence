//! P30 fixed unpacked-array assignment coverage.
//!
//! Each fixture is lowered and executed in both optimizer modes so array
//! snapshot semantics and notifications cannot diverge between pipelines.

#[path = "support/sim.rs"]
mod sim_harness;

use std::path::Path;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

fn run_fixture(file: &str, expected: &str) {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/p30_fixed_arrays")
        .join(file);
    sim_harness::with_frontend_temp_cwd("p30-fixed-arrays", |dir| {
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
                Ok(actual) => failures.push(format!(
                    "{variant}: expected {expected:?}, got {actual:?}"
                )),
                Err(error) => failures.push(format!("{variant}: {error}")),
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(format!("{file}:\n{}", failures.join("\n")))
        }
    })
    .expect("P30 fixed-array conformance");
}

fn reject_fixture(file: &str, diagnostic: &str) {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/p30_fixed_arrays")
        .join(file);
    sim_harness::with_frontend_temp_cwd("p30-fixed-arrays-rejection", |dir| {
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
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map_err(|error| format!("{file}/{variant}: lowering: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| format!("{file}/{variant}: C model build: {error}"))?;
            match sim_harness::run_executable_output(&executable) {
                Ok(output) => {
                    return Err(format!(
                        "{file}/{variant}: shape mismatch unexpectedly succeeded with {:?}",
                        output.stdout
                    ));
                }
                Err(error) if error.contains(diagnostic) => {}
                Err(error) => {
                    return Err(format!("{file}/{variant}: unexpected diagnostic: {error}"));
                }
            }
        }
        Ok(())
    })
    .expect("P30 fixed-array rejection conformance");
}

#[test]
fn fixed_array_assignment_and_views() {
    run_fixture(
        "fixed_array_assignment.sv",
        "PASS fixed_array_assignment\n",
    );
}

#[test]
fn fixed_array_declaration_assignment() {
    run_fixture(
        "fixed_array_declaration_assignment.sv",
        "PASS fixed_array_declaration\n",
    );
}

#[test]
fn fixed_array_dynamic_shape_mismatch_is_runtime_error() {
    reject_fixture(
        "fixed_array_shape_mismatch.sv",
        "fixed unpacked-array assignment size mismatch",
    );
}
