//! Continuation coverage for replicated values, membership, memory and wired nets.
use crate::sim_cli;

#[test]
fn replicated_patterns_preserve_order_state_and_value_contexts() {
    sim_cli::run_case_with_args(
        "continuation_16_19",
        "replicated_contexts",
        "REPLICATED_CONTEXTS_PASS\n",
        "",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "continuation_16_19",
        "replicated_negative_count",
        "value must be positive",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "continuation_16_19",
        "replicated_nested_shape",
        "assignment pattern",
        &["--edition", "2009"],
    );
}

#[test]
fn replicated_pattern_capture_preserves_unexpanded_count_and_element_slots() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    let database = {
        let output = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "replication.sv",
                include_str!("fixtures/sim/continuation_16_19/replicated_contexts.sv"),
            )],
            &compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .expect("legal replicated pattern contexts");
        db::Db::from_slang(&output.snapshot).expect("owned replicated pattern graph")
    };
    database.validate().unwrap();
    let mut pair_patterns = 0;
    for node in database.nodes() {
        if let db::NodeKind::Expr(db::ExprKind::Operation {
            op: db::Operation::MultiAssignmentPattern,
            operands,
            ..
        }) = &node.kind
        {
            assert!(!operands.is_empty(), "count is an owned operand");
            if operands.len() == 3 {
                pair_patterns += 1;
            }
        }
    }
    assert!(
        pair_patterns > 0,
        "the syntactic pair is retained, not a flattened six-element expansion"
    );
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("replicated runtime contexts after native snapshot destruction");
    }
}

#[test]
fn inside_values_keep_array_element_types_and_singular_casts() {
    sim_cli::run_case_with_args(
        "continuation_16_19",
        "inside_value_contexts",
        "INSIDE_VALUE_CONTEXTS_PASS\n",
        "",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "continuation_16_19",
        "inside_aggregate_error",
        "for inside expression",
        &["--edition", "2009"],
    );
}

#[test]
fn inside_value_graph_lowers_after_native_snapshot_destruction() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    let database = {
        let output = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "inside.sv",
                include_str!("fixtures/sim/continuation_16_19/inside_value_contexts.sv"),
            )],
            &compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .expect("legal fixed-array and packed-cast membership");
        db::Db::from_slang(&output.snapshot).unwrap()
    };
    database.validate().unwrap();
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("typed membership after native snapshot destruction");
    }
}

#[test]
fn memory_tokens_and_partial_address_errors_work_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_files(
            "continuation_16_19",
            "memory_tokens",
            "MEMORY_TOKENS_PASS\n",
            "",
            &[],
            &["--edition", edition],
            &[
                ("tokens.hex", "/* based digits */ x z 0x x7 0z z0 f 1_ff\n"),
                ("tokens.bin", "// binary digits\nx z 0x x1 0z z0 1 1_01\n"),
            ],
        );
        sim_cli::run_case_with_files(
            "continuation_16_19", "memory_partial_error", "retained=aa 0b 0c 0d\n",
            "llg: memory file `bad_address.hex`: address jump is outside the destination memory or selected range; load terminated\n",
            &[], &["--edition", edition], &[("bad_address.hex", "aa @8 bb @1 cc\n")],
        );
    }
}

#[test]
fn sparse_memory_warning_and_negative_declared_ranges_keep_edition_policy() {
    for (edition, warning, directions) in [
        ("2001", "llg: memory file `sparse.hex`: memory file contains too few words for the selected range\n",
         "omitted=11 22 33\nstarted=00 aa bb\nexplicit=11 22 33\n"),
        ("2009", "", "omitted=33 22 11\nstarted=bb aa 00\nexplicit=11 22 33\n"),
    ] {
        sim_cli::run_case_with_files(
            "continuation_16_19", "memory_sparse", "sparse=07 aa 07 bb\n", warning, &[],
            &["--edition", edition], &[("sparse.hex", "@2 aa @0 bb\n")],
        );
        let extra_warning = if edition == "2001" {
            "llg: memory file `sparse.hex`: memory file contains more words than the selected range\n"
        } else { "" };
        sim_cli::run_case_with_files(
            "continuation_16_19", "memory_sparse", "sparse=07 07 07 ee\n", extra_warning, &[],
            &["--edition", edition], &[("sparse.hex", "@0 aa @0 bb @0 cc @0 dd @0 ee\n")],
        );
        sim_cli::run_case_with_files(
            "continuation_16_19", "memory_directions", directions, "", &[],
            &["--edition", edition], &[("three.hex", "11 22 33\n"), ("two.hex", "aa bb\n")],
        );
    }
}

#[test]
fn two_state_enum_load_checks_full_numeric_word_before_truncation() {
    sim_cli::run_case_with_files(
        "continuation_16_19", "memory_enum_conversion", "MEMORY_ENUM_CONVERSION_PASS\n",
        concat!(
            "llg: memory file `enum.hex`: numeric memory data does not fit the enum base type; load terminated\n",
            "llg: memory file `two_state.hex`: X/Z memory data converted to a two-state element\n",
            "llg: memory file `enum_unknown.hex`: X/Z memory data converted to a two-state element\n",
        ),
        &[], &["--edition", "2009"],
        &[
            ("enum.hex", "1 4x 0\n"),
            ("signed.hex", "ff 0\n"),
            ("two_state.hex", "x7 z1\n"),
            ("enum_unknown.hex", "x 1 z\n"),
        ],
    );
}

#[test]
fn hierarchical_wired_sites_match_tables_and_port_resolution_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            "continuation_16_19",
            "wired_matrix",
            "WIRED_MATRIX_PASS\n",
            "",
            &[],
            &["--edition", edition],
        );
        sim_cli::run_case_with_args(
            "continuation_16_19",
            "wired_upward_ports",
            "WIRED_UPWARD_PORTS_PASS\n",
            "",
            &[],
            &["--edition", edition],
        );
        sim_cli::reject_case_with_args(
            "net_resolution",
            "hierarchical_procedural_net",
            "cannot assign to a net within a procedural context",
            &["--edition", edition],
        );
    }
}

#[test]
fn hierarchical_wired_graph_lowers_after_native_snapshot_destruction() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    for source in [
        include_str!("fixtures/sim/continuation_16_19/wired_matrix.sv"),
        include_str!("fixtures/sim/continuation_16_19/wired_upward_ports.sv"),
    ] {
        let database = {
            let output = compile::compile_sources_checked(
                &[compile::OwnedSource::compilation_unit("wired.sv", source)],
                &compile::CompileOpts {
                    top: Some("tb".to_owned()),
                    ..Default::default()
                },
            )
            .expect("legal hierarchical driver sources");
            db::Db::from_slang(&output.snapshot).unwrap()
        };
        database.validate().unwrap();
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .expect("hierarchical wired contribution lowering after native teardown");
        }
    }
}
