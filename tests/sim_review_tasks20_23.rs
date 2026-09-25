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
