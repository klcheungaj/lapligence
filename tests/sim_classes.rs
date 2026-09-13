//! End-to-end coverage for the bounded class-object runtime subset.
//!
//! Fixtures are compiled through the owned database, lowered twice, and run
//! through CMake.  Keeping the HDL in checked-in fixtures makes the source
//! provenance and exact optimizer-parity oracle reviewable.

#[path = "support/sim.rs"]
mod sim_harness;

use std::path::Path;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

const FIXTURE_DIR: &str = "tests/fixtures/sim/classes";

fn fixture(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(FIXTURE_DIR)
        .join(name)
}

fn with_compiled_fixture<T>(
    source_name: &str,
    top: &str,
    tag: &str,
    action: impl FnOnce(&std::path::Path, &Db) -> Result<T, String>,
) -> Result<T, String> {
    sim_harness::with_frontend_temp_cwd(tag, |dir| {
        let source_fixture = fixture(source_name);
        let source = dir.join(source_name);
        std::fs::copy(&source_fixture, &source)
            .map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some(top.to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile {source_name}: {error}"))?;
        let database = Db::from_slang(&compiled.snapshot)
            .map_err(|error| format!("database {source_name}: {error}"))?;
        action(dir, &database)
    })
}

#[test]
fn class_objects_constructors_methods_and_aliases_match_across_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }

    let expected = concat!(
        "first=33 alias=33 second=38 twice=66 ratio=1.500000\n",
        "add=5\n",
        "first_after=35 alias_after=35 total=2\n",
    );

    with_compiled_fixture("basic.sv", "tb", "classes-basic", |dir, database| {
        let mut outputs = Vec::new();
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let generated = sim::codegen::generate_from_db_with_opts(database, &options)
                .map_err(|error| format!("{variant} lowering: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", generated.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} C model build: {error}"))?;
            let actual = sim_harness::run_executable(&executable)
                .map_err(|error| format!("{variant} execution: {error}"))?;
            if actual != expected {
                return Err(format!("{variant}: expected {expected:?}, got {actual:?}"));
            }
            outputs.push(actual);
        }
        if outputs[0] != outputs[1] {
            return Err("optimizer changed class semantics".to_owned());
        }
        Ok(())
    })
    .expect("class fixture should compile, build, and execute");
}

#[test]
fn null_class_handle_access_fails_at_runtime_in_both_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }

    with_compiled_fixture("null_access.sv", "tb", "classes-null", |dir, database| {
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let generated = sim::codegen::generate_from_db_with_opts(database, &options)
                .map_err(|error| format!("{variant} lowering: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", generated.model_c.as_str())],
            )
            .map_err(|error| format!("{variant} C model build: {error}"))?;
            let error = sim_harness::run_executable_output(&executable)
                .expect_err("null class access must terminate the model");
            if !error.contains("llg: null class handle access: Box.value") {
                return Err(format!(
                    "{variant}: unexpected null-handle diagnostic: {error}"
                ));
            }
        }
        Ok(())
    })
    .expect("null class fixture should report a runtime failure");
}

#[test]
fn inherited_virtual_methods_casts_and_specializations_match_across_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }

    let expected = concat!(
        "dispatch=28 static=4 base_field=3 super=14\n",
        "widths=15/170\n",
        "constructors=2/7\n",
        "cast=1 down=28\n",
        "bad_cast=0 bad_null=0\n",
        "null_cast=0 null=0\n",
        "null_literal_cast=0 null=0\n",
    );
    with_compiled_fixture(
        "inheritance.sv",
        "tb",
        "classes-inheritance",
        |dir, database| {
            let mut outputs = Vec::new();
            for (variant, options) in [
                ("unoptimized", OptConfig::none()),
                ("optimized", OptConfig::default()),
            ] {
                let generated = sim::codegen::generate_from_db_with_opts(database, &options)
                    .map_err(|error| format!("{variant} lowering: {error}"))?;
                let executable = sim::build::build_model_cmake(
                    &dir.join(variant),
                    &[("model.c", generated.model_c.as_str())],
                )
                .map_err(|error| format!("{variant} C model build: {error}"))?;
                let actual = sim_harness::run_executable(&executable)
                    .map_err(|error| format!("{variant} execution: {error}"))?;
                if actual != expected {
                    return Err(format!("{variant}: expected {expected:?}, got {actual:?}"));
                }
                outputs.push(actual);
            }
            if outputs[0] != outputs[1] {
                return Err("optimizer changed inherited class semantics".to_owned());
            }
            Ok(())
        },
    )
    .expect("inheritance fixture should compile, build, and execute");
}

#[test]
fn pure_virtual_and_out_of_block_methods_match_across_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }

    let expected = "direct=11 dynamic=13\n";
    with_compiled_fixture(
        "pure_virtual.sv",
        "tb",
        "classes-pure-virtual",
        |dir, database| {
            let mut outputs = Vec::new();
            for (variant, options) in [
                ("unoptimized", OptConfig::none()),
                ("optimized", OptConfig::default()),
            ] {
                let generated = sim::codegen::generate_from_db_with_opts(database, &options)
                    .map_err(|error| format!("{variant} lowering: {error}"))?;
                let executable = sim::build::build_model_cmake(
                    &dir.join(variant),
                    &[("model.c", generated.model_c.as_str())],
                )
                .map_err(|error| format!("{variant} C model build: {error}"))?;
                let actual = sim_harness::run_executable(&executable)
                    .map_err(|error| format!("{variant} execution: {error}"))?;
                if actual != expected {
                    return Err(format!("{variant}: expected {expected:?}, got {actual:?}"));
                }
                outputs.push(actual);
            }
            if outputs[0] != outputs[1] {
                return Err("optimizer changed pure virtual class semantics".to_owned());
            }
            Ok(())
        },
    )
    .expect("pure virtual fixture should compile, build, and execute");
}

#[test]
fn forward_class_typedef_and_const_property_match_across_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }

    let expected = "value=6\n";
    with_compiled_fixture(
        "forward_const.sv",
        "tb",
        "classes-forward-const",
        |dir, database| {
            for (variant, options) in [
                ("unoptimized", OptConfig::none()),
                ("optimized", OptConfig::default()),
            ] {
                let generated = sim::codegen::generate_from_db_with_opts(database, &options)
                    .map_err(|error| format!("{variant} lowering: {error}"))?;
                let executable = sim::build::build_model_cmake(
                    &dir.join(variant),
                    &[("model.c", generated.model_c.as_str())],
                )
                .map_err(|error| format!("{variant} C model build: {error}"))?;
                let actual = sim_harness::run_executable(&executable)
                    .map_err(|error| format!("{variant} execution: {error}"))?;
                if actual != expected {
                    return Err(format!("{variant}: expected {expected:?}, got {actual:?}"));
                }
            }
            Ok(())
        },
    )
    .expect("forward typedef/const fixture should compile, build, and execute");
}

#[test]
fn abstract_class_construction_is_rejected_by_frontend() {
    let source_fixture = fixture("abstract_new.sv");
    sim_harness::with_frontend_temp_cwd("classes-abstract-new", |dir| {
        let source = dir.join("abstract_new.sv");
        std::fs::copy(&source_fixture, &source)
            .map_err(|error| format!("copy fixture: {error}"))?;
        let error = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .expect_err("constructing a virtual class must be rejected");
        let diagnostics = error
            .diagnostics()
            .map(|diagnostics| {
                diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.message.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| error.to_string());
        if !diagnostics.contains("cannot create instance of virtual class") {
            return Err(format!(
                "unexpected abstract-class diagnostic: {diagnostics}"
            ));
        }
        Ok(())
    })
    .expect("abstract class construction should produce a frontend diagnostic");
}
