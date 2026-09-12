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
    sim_harness::with_frontend_temp_cwd("data-types-next", |dir| {
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
    sim_harness::with_frontend_temp_cwd("data-types-next-rejection", |dir| {
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

        let database = Db::from_slang(&compiled.snapshot)
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

#[test]
fn distinct_unpacked_struct_typedefs_are_not_assignment_compatible() {
    let source = r#"module tb;
    typedef struct { logic [7:0] value; } left_t;
    typedef struct { logic [7:0] value; } right_t;
    left_t left;
    right_t right;
    initial left = right;
endmodule
"#;
    sim_harness::with_frontend_temp_cwd("unpacked-nominal-mismatch", |dir| {
        let source_path = dir.join("tb.sv");
        std::fs::write(&source_path, source).map_err(|error| error.to_string())?;
        let options = compile::CompileOpts {
            files: vec![source_path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        };
        let partial = compile::compile(&options).map_err(|error| error.to_string())?;
        if partial.ok() {
            return Err("Slang accepted assignment between distinct unpacked typedefs".into());
        }
        if !matches!(
            compile::compile_checked(&options),
            Err(compile::CompileError::FrontendDiagnostics(_))
        ) {
            return Err("checked compilation did not withhold the invalid design".into());
        }
        Ok(())
    })
    .expect("distinct unpacked typedefs must remain nominally incompatible");
}

#[test]
fn packed_nominal_type_key_mismatch_is_rejected() {
    let source = r#"module tb;
    typedef struct packed { logic [7:0] value; } left_lane_t;
    typedef struct packed { logic [7:0] value; } right_lane_t;
    typedef struct packed { left_lane_t lane; } holder_t;
    holder_t value = '{right_lane_t: 8'hff, default: '0};
endmodule
"#;
    sim_harness::with_frontend_temp_cwd("packed-nominal-key-mismatch", |dir| {
        let source_path = dir.join("tb.sv");
        std::fs::write(&source_path, source).map_err(|error| error.to_string())?;
        let options = compile::CompileOpts {
            files: vec![source_path.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        };
        let compiled = compile::compile_checked(&options).map_err(|error| error.to_string())?;
        let database = Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?;
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let error = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map(|_| "generated successfully".to_owned())
                .unwrap_or_else(|error| error.to_string());
            if !error.contains("no matching member or type") {
                return Err(format!(
                    "{variant}: distinct same-width nominal key was not rejected: {error}"
                ));
            }
        }
        Ok(())
    })
    .expect("same-width packed nominal type keys must not match by width");
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
    packed_aggregate_nested_selections,
    "packed_aggregate_selections.sv",
    "packed_aggregate_selections"
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
    inside_aggregate_container_real_and_string_contexts,
    "inside_aggregate_contexts.sv",
    "inside_aggregate_contexts"
);

#[test]
fn inside_chandle_context_is_rejected_as_one_frontend_fault() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_next")
        .join("inside_chandle_rejected.sv");
    sim_harness::with_frontend_temp_cwd("data-types-next-inside-rejection", |dir| {
        let source = dir.join("inside_chandle_rejected.sv");
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        if !compiled.ok() {
            let diagnostic = format!("{:?}", compiled.diagnostics).to_ascii_lowercase();
            if diagnostic.contains("chandle") && diagnostic.contains("inside") {
                return Ok(());
            }
            return Err(format!("unexpected inside diagnostic: {diagnostic}"));
        }
        let database =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let error = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map(|_| "generated successfully".to_owned())
                .unwrap_or_else(|error| error.to_string())
                .to_ascii_lowercase();
            if !error.contains("chandle") || !error.contains("inside") {
                return Err(format!("{variant}: unexpected inside result: {error}"));
            }
        }
        Ok(())
    })
    .expect("chandle inside operand must be rejected");
}
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
datatype_case!(
    mixed_subprogram_lifetimes_are_reentrant,
    "mixed_subprogram_lifetimes.sv",
    "mixed_subprogram_lifetimes"
);
#[test]
fn static_function_runtime_initializer_runs_once_before_processes() {
    run_fixture(
        "static_function_runtime_initializer.sv",
        "static_function_runtime_initializer",
    );
}
datatype_case!(
    static_local_storage_is_per_instance,
    "static_local_multiple_instances.sv",
    "static_local_multiple_instances"
);
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
    dynamic_value_array_copy_resize_and_delete,
    "dynamic_value_arrays.sv",
    "dynamic_value_arrays"
);
datatype_case!(
    nested_dynamic_array_copy_resize_and_delete,
    "nested_dynamic_arrays.sv",
    "nested_dynamic_arrays"
);

#[test]
fn dynamic_array_negative_runtime_size_is_rejected_in_both_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_next")
        .join("dynamic_array_invalid_size.sv");
    sim_harness::with_frontend_temp_cwd("data-types-next-negative-size", |dir| {
        let source = dir.join("dynamic_array_invalid_size.sv");
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile_checked(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        let database =
            Db::from_slang(&compiled.snapshot).map_err(|error| format!("database: {error}"))?;
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let model = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map_err(|error| format!("{variant}: lowering: {error}"))?;
            let executable = sim::build::build_model_cmake(
                &dir.join(variant),
                &[("model.c", model.model_c.as_str())],
            )
            .map_err(|error| format!("{variant}: C model build: {error}"))?;
            let output = std::process::Command::new(&executable)
                .output()
                .map_err(|error| format!("{variant}: execute: {error}"))?;
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.success()
                || !stderr.contains("dynamic-array size is unknown or negative")
            {
                return Err(format!(
                    "{variant}: expected negative-size rejection, status {:?}, stderr {stderr:?}",
                    output.status
                ));
            }
        }
        Ok(())
    })
    .expect("negative dynamic-array size must be rejected");
}
datatype_case!(
    associative_array_insert_traverse_and_delete,
    "associative_array.sv",
    "associative_array"
);
datatype_case!(
    associative_array_defaults_copy_and_wildcard_keys,
    "associative_array_p33.sv",
    "associative_array_p33"
);
datatype_case!(
    generic_queue_and_associative_values,
    "generic_containers_p32_p33.sv",
    "generic_containers_p32_p33"
);
datatype_case!(
    shortreal_container_elements_apply_destination_rounding,
    "container_copy_conversion.sv",
    "container_copy_conversion"
);
datatype_case!(
    nested_container_values,
    "nested_container_values_p32_p33.sv",
    "nested_container_values_p32_p33"
);
datatype_case!(
    associative_array_key_validation_contexts,
    "container_assignment_contexts.sv",
    "container_assignment_contexts"
);

#[test]
fn wildcard_associative_traversal_is_rejected() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_next")
        .join("associative_array_wildcard_traversal.sv");
    sim_harness::with_frontend_temp_cwd("data-types-next-wildcard-rejection", |dir| {
        let source = dir.join("associative_array_wildcard_traversal.sv");
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let partial = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        if !partial.ok() {
            let diagnostic = format!("{:?}", partial.diagnostics).to_ascii_lowercase();
            if diagnostic.contains("wildcard") || diagnostic.contains("first") {
                return Ok(());
            }
            return Err(format!(
                "unexpected wildcard traversal diagnostic: {diagnostic}"
            ));
        }
        let database =
            Db::from_slang(&partial.snapshot).map_err(|error| format!("database: {error}"))?;
        for options in [OptConfig::none(), OptConfig::default()] {
            let error = sim::codegen::generate_from_db_with_opts(&database, &options)
                .map(|_| "generated successfully".to_owned())
                .unwrap_or_else(|error| error.to_string())
                .to_ascii_lowercase();
            if !error.contains("wildcard") || !error.contains("traversal") {
                return Err(format!("unexpected codegen result: {error}"));
            }
        }
        Ok(())
    })
    .expect("wildcard associative traversal must be rejected");
}
datatype_case!(queue_order_and_methods, "queue.sv", "queue");
datatype_case!(
    queue_slices_and_bounded_overflow,
    "queue_p32.sv",
    "queue_p32"
);
datatype_case!(
    executed_data_and_array_queries,
    "query_functions.sv",
    "query_functions"
);
datatype_case!(runtime_enum_methods, "enum_methods.sv", "enum_methods");

#[test]
fn queue_slice_real_bound_is_rejected() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_next")
        .join("queue_p32_invalid.sv");
    sim_harness::with_frontend_temp_cwd("data-types-next-queue-rejection", |dir| {
        let source = dir.join("queue_p32_invalid.sv");
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let options = compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        };
        let partial = compile::compile(&options).map_err(|error| format!("compile: {error}"))?;
        if !partial.ok() {
            let diagnostic = format!("{:?}", partial.diagnostics).to_ascii_lowercase();
            return if diagnostic.contains("integral") || diagnostic.contains("real") {
                Ok(())
            } else {
                Err(format!("unexpected queue slice diagnostic: {diagnostic}"))
            };
        }
        let compiled =
            compile::compile_checked(&options).map_err(|error| format!("compile: {error}"))?;
        for (variant, options) in [
            ("unoptimized", OptConfig::none()),
            ("optimized", OptConfig::default()),
        ] {
            let error = sim::codegen::generate_from_db_with_opts(
                &Db::from_slang(&compiled.snapshot).map_err(|error| error.to_string())?,
                &options,
            )
            .map(|_| "generated successfully".to_owned())
            .unwrap_or_else(|error| error.to_string());
            if !error.to_ascii_lowercase().contains("integral") {
                return Err(format!(
                    "{variant}: non-integral queue slice bound was not rejected: {error}"
                ));
            }
        }
        Ok(())
    })
    .expect("queue slice bound rejection");
}
datatype_case!(string_value_and_methods, "string.sv", "string");
datatype_case!(
    string_subroutine_forms,
    "string_subroutine_forms.sv",
    "string_subroutine_forms"
);
datatype_case!(
    string_delayed_nba,
    "string_delayed_nba.sv",
    "string_delayed_nba"
);
datatype_case!(
    string_function_early_return_and_self_copy,
    "string_return_packed_input.sv",
    "string_return_packed_input"
);
datatype_case!(chandle_null_assignment_and_calls, "chandle.sv", "chandle");

#[test]
fn array_query_invalid_dimension_is_rejected_as_one_frontend_fault() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/data_types_next")
        .join("query_functions_invalid_dimension.sv");
    sim_harness::with_frontend_temp_cwd("data-types-next-query-rejection", |dir| {
        let source = dir.join("query_functions_invalid_dimension.sv");
        std::fs::copy(&fixture, &source).map_err(|error| format!("copy fixture: {error}"))?;
        let compiled = compile::compile(&compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        })
        .map_err(|error| format!("compile: {error}"))?;
        if compiled.ok() {
            return Err("invalid array-query dimension was accepted".to_owned());
        }
        let diagnostic = format!("{:?}", compiled.diagnostics).to_ascii_lowercase();
        if !diagnostic.contains("dimension") || !diagnostic.contains("invalid") {
            return Err(format!("unexpected array-query diagnostic: {diagnostic}"));
        }
        Ok(())
    })
    .expect("invalid array-query dimension must be rejected");
}
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
