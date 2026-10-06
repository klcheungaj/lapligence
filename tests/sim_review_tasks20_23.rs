//! Continuation coverage for typed destinations, values, calls and continuous drivers.
use crate::sim_cli;

#[test]
fn positional_deconstruction_captures_sources_and_all_targets_before_writes() {
    sim_cli::run_case_with_args(
        "continuation_20_23",
        "pattern_capture",
        "PATTERN_CAPTURE_PASS\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::run_case_with_args(
        "continuation_20_23",
        "pattern_layouts",
        "PATTERN_LAYOUTS_PASS\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn positional_deconstruction_retains_illegal_target_diagnostics() {
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues",
        "syn_003_keyed_lvalue",
        "expression is not assignable",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues",
        "syn_003_replicated_lvalue",
        "expression is not assignable",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues",
        "syn_003_width_mismatch",
        "assignment-pattern lvalue target",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues",
        "syn_003_automatic_nba",
        "automatic assignment-pattern target",
        &["--edition", "sv2009"],
    );
}

#[test]
fn positional_deconstruction_lowers_after_native_snapshot_destruction() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    let database = {
        let result = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "pattern_capture.sv",
                include_str!("fixtures/sim/continuation_20_23/pattern_capture.sv"),
            )],
            &compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .expect("legal positional destination captures");
        db::Db::from_slang(&result.snapshot).unwrap()
    };
    database.validate().unwrap();
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("owned positional targets remain executable after native teardown");
    }
}

#[test]
fn fixed_layouts_preserve_nested_values_across_limb_boundaries() {
    for width in [1, 7, 8, 31, 32, 33, 63, 64, 65, 129] {
        let define = format!("CONTINUATION_LAYOUT_W={width}");
        sim_cli::run_case_with_args(
            "continuation_20_23",
            "layout_contexts",
            &format!("LAYOUT_CONTEXTS_PASS W={width}\n"),
            "",
            &[],
            &["--edition", "sv2009", "--define", &define],
        );
    }
    sim_cli::reject_case_with_args(
        "syn012_fixed_layout",
        "packed_union_width_rejected",
        "same width",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn012_fixed_layout",
        "nominal_record_mismatch_rejected",
        "no implicit conversion",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn012_fixed_layout",
        "unpacked_union_bitstream_rejected",
        "invalid casting type",
        &["--edition", "sv2009"],
    );
}

#[test]
fn fixed_layout_graph_retains_nominal_shapes_after_native_teardown() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    let database = {
        let result = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "layout_contexts.sv",
                include_str!("fixtures/sim/continuation_20_23/layout_contexts.sv"),
            )],
            &compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .expect("legal fixed nested values");
        db::Db::from_slang(&result.snapshot).unwrap()
    };
    database.validate().unwrap();
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("all descriptor projections use owned type data");
    }
}

#[test]
fn finite_calls_keep_static_outputs_copyin_and_reference_identity_distinct() {
    sim_cli::run_case_with_args(
        "continuation_20_23",
        "static_output_values",
        "STATIC_OUTPUT_VALUES_PASS\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::run_case_with_args(
        "continuation_20_23",
        "call_contexts",
        "CALL_CONTEXTS_PASS\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues",
        "syn_003_ref_nba",
        "automatic assignment-pattern target",
        &["--edition", "sv2009"],
    );
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            "syn013_zero_time_calls",
            "legacy_calls",
            "legacy value=40 result=42 calls=2\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn static_output_formals_and_defaults_lower_after_native_teardown() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    for (name, source) in [
        (
            "static_output_values.sv",
            include_str!("fixtures/sim/continuation_20_23/static_output_values.sv"),
        ),
        (
            "call_contexts.sv",
            include_str!("fixtures/sim/continuation_20_23/call_contexts.sv"),
        ),
    ] {
        let database = {
            let result = compile::compile_sources_checked(
                &[compile::OwnedSource::compilation_unit(name, source)],
                &compile::CompileOpts {
                    top: Some("tb".to_owned()),
                    ..Default::default()
                },
            )
            .expect("legal zero-time fixed calls");
            db::Db::from_slang(&result.snapshot).unwrap()
        };
        database.validate().unwrap();
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .expect("call storage and defaults use owned data");
        }
    }
}

#[test]
fn continuous_arrays_keep_values_dependencies_and_static_pattern_topology() {
    sim_cli::run_case_with_args(
        "continuation_20_23",
        "continuous_contexts",
        "CONTINUOUS_CONTEXTS_PASS\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::run_case_with_args(
        "continuation_20_23",
        "continuous_rhs_once",
        "CONTINUOUS_RHS_EVAL\nCONTINUOUS_RHS_EVAL\nCONTINUOUS_RHS_EVAL\nCONTINUOUS_RHS_ONCE_PASS\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::run_case_with_args(
        "continuation_20_23",
        "continuous_identity",
        "CONTINUOUS_IDENTITY_PASS\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::run_case_with_args(
        "continuation_20_23",
        "continuous_force_control",
        "CONTINUOUS_FORCE_CONTROL_PASS\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn continuous_array_conflicts_remain_errors() {
    sim_cli::reject_case_with_args(
        "rtl_completion",
        "syn_006_array_continuous_variable_conflict",
        "multiple continuous assignments to variable storage",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "continuation_20_23",
        "continuous_mixed_writer_error",
        "has both a continuous assignment",
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "continuation_20_23",
        "continuous_initialized_writer_error",
        "has both a continuous assignment",
        &["--edition", "sv2009"],
    );
}

#[test]
fn continuous_array_graph_lowers_without_a_native_snapshot() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    for (name, source) in [
        (
            "continuous_contexts.sv",
            include_str!("fixtures/sim/continuation_20_23/continuous_contexts.sv"),
        ),
        (
            "continuous_rhs_once.sv",
            include_str!("fixtures/sim/continuation_20_23/continuous_rhs_once.sv"),
        ),
    ] {
        let database = {
            let result = compile::compile_sources_checked(
                &[compile::OwnedSource::compilation_unit(name, source)],
                &compile::CompileOpts {
                    top: Some("tb".to_owned()),
                    ..Default::default()
                },
            )
            .expect("legal continuous fixed-value contexts");
            db::Db::from_slang(&result.snapshot).unwrap()
        };
        database.validate().unwrap();
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .expect("typed continuous paths after native destruction");
        }
    }
}
