//! Continuation coverage for replicated values, membership, memory and wired nets.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn replicated_patterns_preserve_order_state_and_value_contexts() {
    sim_cli::run_case_with_args(
        "continuation_16_19", "replicated_contexts", "REPLICATED_CONTEXTS_PASS\n", "", &[],
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "continuation_16_19", "replicated_negative_count", "value must be positive",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "continuation_16_19", "replicated_nested_shape", "assignment pattern",
        &["--edition", "2009"],
    );
}

#[test]
fn replicated_pattern_capture_preserves_unexpanded_count_and_element_slots() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    let database = {
        let output = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit("replication.sv",
                include_str!("fixtures/sim/continuation_16_19/replicated_contexts.sv"))],
            &compile::CompileOpts { top: Some("tb".to_owned()), ..Default::default() },
        ).expect("legal replicated pattern contexts");
        db::Db::from_slang(&output.snapshot).expect("owned replicated pattern graph")
    };
    database.validate().unwrap();
    let mut pair_patterns = 0;
    for node in database.nodes() {
        if let db::NodeKind::Expr(db::ExprKind::Operation {
            op: db::Operation::MultiAssignmentPattern, operands, ..
        }) = &node.kind {
            assert!(!operands.is_empty(), "count is an owned operand");
            if operands.len() == 3 {
                pair_patterns += 1;
            }
        }
    }
    assert!(pair_patterns > 0, "the syntactic pair is retained, not a flattened six-element expansion");
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("replicated runtime contexts after native snapshot destruction");
    }
}
