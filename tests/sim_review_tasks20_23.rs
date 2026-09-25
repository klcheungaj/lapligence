//! Continuation coverage for typed destinations, values, calls and continuous drivers.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn positional_deconstruction_captures_sources_and_all_targets_before_writes() {
    sim_cli::run_case_with_args(
        "continuation_20_23", "pattern_capture", "PATTERN_CAPTURE_PASS\n", "", &[],
        &["--edition", "2009"],
    );
    sim_cli::run_case_with_args(
        "continuation_20_23", "pattern_layouts", "PATTERN_LAYOUTS_PASS\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn positional_deconstruction_retains_illegal_target_diagnostics() {
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues", "syn_003_keyed_lvalue", "expression is not assignable",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues", "syn_003_replicated_lvalue", "expression is not assignable",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues", "syn_003_width_mismatch", "assignment-pattern lvalue target",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues", "syn_003_automatic_nba", "automatic assignment-pattern target",
        &["--edition", "2009"],
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
            &compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
        ).expect("legal positional destination captures");
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
            "continuation_20_23", "layout_contexts", &format!("LAYOUT_CONTEXTS_PASS W={width}\n"),
            "", &[], &["--edition", "2009", "--define", &define],
        );
    }
    sim_cli::reject_case_with_args(
        "syn012_fixed_layout", "packed_union_width_rejected", "same width",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn012_fixed_layout", "nominal_record_mismatch_rejected", "no implicit conversion",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn012_fixed_layout", "unpacked_union_bitstream_rejected", "invalid casting type",
        &["--edition", "2009"],
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
            &compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
        ).expect("legal fixed nested values");
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
        "continuation_20_23", "static_output_values", "STATIC_OUTPUT_VALUES_PASS\n", "", &[],
        &["--edition", "2009"],
    );
    sim_cli::run_case_with_args(
        "continuation_20_23", "call_contexts", "CALL_CONTEXTS_PASS\n", "", &[],
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "syn003_pattern_lvalues", "syn_003_ref_nba", "automatic assignment-pattern target",
        &["--edition", "2009"],
    );
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            "syn013_zero_time_calls", "legacy_calls", "legacy value=40 result=42 calls=2\n",
            "", &[], &["--edition", edition],
        );
    }
}

#[test]
fn static_output_formals_and_defaults_lower_after_native_teardown() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    for (name, source) in [
        ("static_output_values.sv", include_str!("fixtures/sim/continuation_20_23/static_output_values.sv")),
        ("call_contexts.sv", include_str!("fixtures/sim/continuation_20_23/call_contexts.sv")),
    ] {
        let database = {
            let result = compile::compile_sources_checked(
                &[compile::OwnedSource::compilation_unit(name, source)],
                &compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
            ).expect("legal zero-time fixed calls");
            db::Db::from_slang(&result.snapshot).unwrap()
        };
        database.validate().unwrap();
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .expect("call storage and defaults use owned data");
        }
    }
}
