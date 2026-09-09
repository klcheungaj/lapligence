//! Supplementary HDL datatype edge cases with checked-in, self-checking
//! fixtures. Each case executes with optimization disabled and enabled.

#[path = "support/sim.rs"]
mod sim_harness;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};
use std::path::Path;

fn run_fixture(file: &str, label: &str, width: usize) {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_type_edges")
        .join(file);
    let expected = format!("PASS {label} WIDTH={width}\n");
    sim_harness::with_frontend_temp_cwd("data-type-edges", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            param_overrides: vec![format!("WIDTH={width}")],
            ..Default::default()
        })
        .map_err(|error| format!("{file}, width {width}: compile: {error}"))?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;

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
    .expect("supplementary datatype edge conformance");
}

fn check_source_less_cast_result(
    result: Result<sim::codegen::GeneratedModel, String>,
    build_dir: &Path,
    variant: &str,
    expected: &str,
) -> Result<(), String> {
    match result {
        Ok(model) => {
            let executable =
                sim::build::build_model_cmake(build_dir, &[("model.c", model.model_c.as_str())])
                    .map_err(|error| format!("{variant} C model build: {error}"))?;
            let actual = sim_harness::run_executable(&executable)?;
            if actual == expected {
                Ok(())
            } else {
                Err(format!(
                    "{variant}: successful source-less lowering produced wrong semantics: expected {expected:?}, got {actual:?}"
                ))
            }
        }
        Err(error) => {
            let normalized = error.to_ascii_lowercase();
            if normalized.contains("source") && normalized.contains("cast") {
                Ok(())
            } else {
                Err(format!(
                    "{variant}: source-less size cast must either run correctly or explicitly diagnose missing cast source: {error}"
                ))
            }
        }
    }
}

macro_rules! datatype_edge_case {
    ($name:ident, $file:literal, $label:literal, $width:expr) => {
        #[test]
        fn $name() {
            run_fixture($file, $label, $width);
        }
    };
}

datatype_edge_case!(
    select_edges_4096_bits,
    "select_edges.sv",
    "select_edges",
    4096
);
datatype_edge_case!(
    select_edges_65536_bits,
    "select_edges.sv",
    "select_edges",
    65536
);
datatype_edge_case!(
    procedural_lhs_indices_preserve_96_bit_expression,
    "wide_lhs_indices.sv",
    "wide_lhs_indices",
    96
);
datatype_edge_case!(
    indexed_part_assignment_uses_selected_width_and_clips,
    "indexed_part_assignment_context.sv",
    "indexed_part_assignment_context",
    128
);
datatype_edge_case!(
    signed_packed_members_preserve_member_type,
    "signed_packed_members.sv",
    "signed_packed_members",
    16
);
datatype_edge_case!(
    packed_state_aggregates_128_bits,
    "packed_state_aggregates.sv",
    "packed_state_aggregates",
    128
);
datatype_edge_case!(
    packed_state_aggregates_4096_bits,
    "packed_state_aggregates.sv",
    "packed_state_aggregates",
    4096
);
datatype_edge_case!(
    enum_state_defaults_128_bits,
    "enum_state_defaults.sv",
    "enum_state_defaults",
    128
);
datatype_edge_case!(
    enum_state_defaults_4096_bits,
    "enum_state_defaults.sv",
    "enum_state_defaults",
    4096
);
datatype_edge_case!(
    numeric_size_casts_source_enabled_4096_bits,
    "numeric_size_casts.sv",
    "numeric_size_casts",
    4096
);
datatype_edge_case!(
    real_to_packed_128_bits,
    "real_to_wide.sv",
    "real_to_wide",
    128
);
datatype_edge_case!(
    real_to_packed_4096_bits,
    "real_to_wide.sv",
    "real_to_wide",
    4096
);
datatype_edge_case!(
    two_state_subprograms_128_bits,
    "two_state_subprograms.sv",
    "two_state_subprograms",
    128
);
datatype_edge_case!(
    two_state_subprograms_4096_bits,
    "two_state_subprograms.sv",
    "two_state_subprograms",
    4096
);
datatype_edge_case!(
    recursive_function_with_maximum_storage,
    "recursive_with_max_storage.sv",
    "recursive_with_max_storage",
    1048575
);

#[test]
fn numeric_size_casts_with_bare_db_are_correct_or_explicitly_unsupported() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let file = "numeric_size_casts.sv";
    let width = 128;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_type_edges")
        .join(file);
    let expected = format!("PASS numeric_size_casts WIDTH={width}\n");
    sim_harness::with_frontend_temp_cwd("numeric-size-cast-bare-db", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            param_overrides: vec![format!("WIDTH={width}")],
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        for (variant, options) in [
            ("bare-db-unoptimized", OptConfig::none()),
            ("bare-db-optimized", OptConfig::default()),
        ] {
            check_source_less_cast_result(
                sim::codegen::generate_from_db_with_opts(&db, &options)
                    .map_err(|error| error.to_string()),
                &dir.join(variant),
                variant,
                &expected,
            )?;
        }
        Ok(())
    })
    .expect("bare Db numeric size cast contract");
}

#[test]
fn numeric_size_casts_with_public_generate_are_correct_or_explicitly_unsupported() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let file = "numeric_size_casts.sv";
    let width = 128;
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_type_edges")
        .join(file);
    let expected = format!("PASS numeric_size_casts WIDTH={width}\n");
    sim_harness::with_frontend_temp_cwd("numeric-size-cast-public-generate", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            param_overrides: vec![format!("WIDTH={width}")],
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let db = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        check_source_less_cast_result(
            sim::codegen::generate(&db).map_err(|error| error.to_string()),
            &dir.join("public-generate"),
            "public-generate",
            &expected,
        )
    })
    .expect("public generate numeric size cast contract");
}
