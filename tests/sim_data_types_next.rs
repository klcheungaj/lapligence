//! Independent next-phase SystemVerilog datatype conformance fixtures.
//! Every checked-in fixture runs with optimization disabled and enabled.

#[path = "support/sim.rs"]
mod sim_harness;

use std::path::Path;

use llg::core::{compile, db::Db};
use llg::sim::{self, opt::OptConfig};

fn reject_no_codegen_error(_: &str) -> bool {
    false
}

fn run_fixture_bytes_with_codegen_rejection(
    file: &str,
    expected: &[u8],
    allowed_rejection: fn(&str) -> bool,
) {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_next")
        .join(file);
    sim_harness::with_surelog_temp_cwd("data-types-next", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("{file}: compile: {error}"))?;
        let database = Db::build_with_source_files(
            compiled.uhdm_design().ok_or("no UHDM design")?,
            &compiled.frontend_source_files(),
        )
        .map_err(|error| format!("{file}: database: {error}"))?;

        let mut failures = Vec::new();
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let actual = (|| {
                let model = match sim::codegen::generate_from_db_with_opts(&database, &options) {
                    Ok(model) => model,
                    Err(error) if allowed_rejection(&error.to_string()) => return Ok(None),
                    Err(error) => return Err(format!("lowering: {error}")),
                };
                let executable = sim::build::build_model_cmake(
                    &dir.join(variant),
                    &[("model.c", model.model_c.as_str())],
                )
                .map_err(|error| format!("C model build: {error}"))?;
                sim_harness::run_executable_output(&executable).map(|output| Some(output.stdout))
            })();
            match actual {
                Ok(None) => {}
                Ok(Some(actual)) if actual == expected => {}
                Ok(Some(actual)) => {
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
    .expect("next-phase datatype conformance");
}

fn run_fixture_bytes(file: &str, expected: &[u8]) {
    run_fixture_bytes_with_codegen_rejection(file, expected, reject_no_codegen_error);
}

fn run_fixture(file: &str, label: &str) {
    run_fixture_bytes(file, format!("PASS {label}\n").as_bytes());
}

fn run_fixture_output(file: &str, expected: &str) {
    run_fixture_bytes(file, expected.as_bytes());
}

fn run_rejection_fixture(file: &str) {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_next")
        .join(file);
    sim_harness::with_surelog_temp_cwd("data-types-next-rejection", |dir| {
        let source = dir.join(file);
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("{file}: compile: {error}"))?;
        if !compiled.ok() {
            let diagnostic = format!("{:?}", compiled.diagnostics).to_ascii_lowercase();
            return if diagnostic.contains("strength") && diagnostic.contains("scalar") {
                Ok(())
            } else {
                Err(format!(
                    "{file}: unexpected frontend diagnostic: {diagnostic}"
                ))
            };
        }

        let database = Db::build_with_source_files(
            compiled.uhdm_design().ok_or("no UHDM design")?,
            &compiled.frontend_source_files(),
        )
        .map_err(|error| format!("{file}: database: {error}"))?;
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let error = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map(|_| "generated successfully".to_owned())
                .unwrap_or_else(|error| error.to_string());
            let normalized = error.to_ascii_lowercase();
            if !normalized.contains("strength") || !normalized.contains("scalar") {
                return Err(format!("{file}: {variant}: unexpected result: {error}"));
            }
        }
        Ok(())
    })
    .expect("next-phase datatype rejection conformance");
}

macro_rules! datatype_case {
    ($name:ident, $file:literal, $label:literal) => {
        #[test]
        fn $name() {
            run_fixture($file, $label);
        }
    };
}

datatype_case!(
    packed_union_member_aliasing,
    "packed_union.sv",
    "packed_union"
);
datatype_case!(
    packed_streaming_slice_order,
    "packed_streaming.sv",
    "packed_streaming"
);
datatype_case!(
    inside_scalar_range_unknown_and_signedness,
    "inside_membership.sv",
    "inside_membership"
);
datatype_case!(
    static_function_and_task_locals_persist,
    "static_subprogram_storage.sv",
    "static_subprogram_storage"
);
datatype_case!(
    static_function_first_statement_executes_each_call,
    "static_function_executable_assignments.sv",
    "static_function_executable_assignments"
);
#[test]
fn static_function_initializer_is_once_only_or_explicitly_unsupported() {
    fn explicit_nonconstant_initializer_rejection(error: &str) -> bool {
        let normalized = error.to_ascii_lowercase();
        normalized.contains("static")
            && normalized.contains("initial")
            && (normalized.contains("unsupported") || normalized.contains("not supported"))
            && (normalized.contains("nonconstant")
                || normalized.contains("non-constant")
                || normalized.contains("runtime"))
    }

    run_fixture_bytes_with_codegen_rejection(
        "static_function_runtime_initializer.sv",
        b"PASS static_function_runtime_initializer\n",
        explicit_nonconstant_initializer_rejection,
    );
}
datatype_case!(
    static_task_output_nba_persists,
    "static_task_nba.sv",
    "static_task_nba"
);
datatype_case!(
    unpacked_struct_defaults_members_and_copy,
    "unpacked_struct.sv",
    "unpacked_struct"
);
datatype_case!(
    unpacked_union_members_share_storage,
    "unpacked_union.sv",
    "unpacked_union"
);
datatype_case!(
    dynamic_array_allocate_copy_resize_and_delete,
    "dynamic_array.sv",
    "dynamic_array"
);
datatype_case!(
    associative_array_insert_traverse_and_delete,
    "associative_array.sv",
    "associative_array"
);
datatype_case!(queue_order_and_methods, "queue.sv", "queue");
datatype_case!(string_value_and_methods, "string.sv", "string");
datatype_case!(
    string_function_early_return_and_self_copy,
    "string_return_packed_input.sv",
    "string_return_packed_input"
);
datatype_case!(chandle_null_assignment_and_calls, "chandle.sv", "chandle");
datatype_case!(
    continuous_assignment_drive_strength_resolution,
    "continuous_assignment_strengths.sv",
    "continuous_assignment_strengths"
);
datatype_case!(
    continuous_assignment_highz_strength_endpoints,
    "continuous_assignment_highz_strengths.sv",
    "continuous_assignment_highz_strengths"
);
datatype_case!(
    streaming_lhs_slices_aliasing_and_evaluation,
    "streaming_lhs.sv",
    "streaming_lhs"
);

#[test]
fn string_argument_cast_copy_and_display_conversions() {
    run_fixture_output(
        "string_conversions.sv",
        "STRING-DISPLAY golden\nPASS string_conversions\n",
    );
}

#[test]
fn dynamic_string_escape_bytes_initializers_and_shadowed_formals() {
    run_fixture_bytes(
        "dynamic_string_formatting.sv",
        b"WRITE<N:line1\nline2\tAB\xff>|seed=seed|formal=formal|global=GLOBAL\n\
PASS dynamic_string_formatting\n",
    );
}

datatype_case!(
    packed_container_element_assignment_contexts,
    "container_assignment_contexts.sv",
    "container_assignment_contexts"
);

#[test]
fn continuous_assignment_vector_strength_is_rejected_as_non_scalar() {
    run_rejection_fixture("continuous_assignment_vector_strength_rejected.sv");
}
