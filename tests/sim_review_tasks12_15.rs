//! Continuation coverage for schedule positions twelve through fifteen.
use crate::sim_cli;

fn map_fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/review_library_configs")
        .join(name);
    assert!(path.is_file(), "missing map fixture: {}", path.display());
    path.to_string_lossy().into_owned()
}

#[test]
fn embedded_map_configurations_match_separate_sources_in_both_editions() {
    for edition in ["2001", "2009"] {
        for name in ["root.map", "included.map", "multiple.map"] {
            let map = map_fixture(name);
            sim_cli::run_case_with_args(
                "review_library_configs",
                "top",
                "mapped=22 11\n",
                "",
                &[],
                &[
                    "--edition",
                    edition,
                    "--top",
                    "choose:config",
                    "--libmap",
                    &map,
                ],
            );
        }
        let map = map_fixture("separate.map");
        sim_cli::run_case_with_source_prefix(
            "review_library_configs",
            "top",
            &["config"],
            "mapped=22 11\n",
            "",
            &[],
            &[
                "--edition",
                edition,
                "--top",
                "choose:config",
                "--libmap",
                &map,
            ],
        );
        let map = map_fixture("unterminated.map");
        sim_cli::reject_case_with_args(
            "review_library_configs",
            "top",
            "missing endconfig",
            &[
                "--edition",
                edition,
                "--top",
                "choose:config",
                "--libmap",
                &map,
            ],
        );
    }
}

#[test]
fn logical_map_configuration_changes_reach_owned_hierarchy_after_snapshot_teardown() {
    use llg::core::{compile, db, model};
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        for (first, expected) in [("high", "high"), ("low", "low")] {
            let text = format!(
                "library cells rtl/cells.sv;\nconfig choose; design custom.top; \
                 cell selected_leaf use cells.low; instance top.u use cells.{first}; endconfig\n"
            );
            let database = {
                let output = compile::compile_sources_checked(
                    &[
                        compile::OwnedSource::compilation_unit(
                            "virtual/top.sv",
                            "module top; selected_leaf u(); selected_leaf v(); endmodule",
                        ),
                        compile::OwnedSource::include(
                            "virtual/rtl/cells.sv",
                            "module low; endmodule module high; endmodule",
                        ),
                    ],
                    &compile::CompileOpts {
                        top: Some("choose:config".to_owned()),
                        edition,
                        default_library: Some("custom".to_owned()),
                        library_maps: vec![compile::OwnedSource::include(
                            "virtual/root.map",
                            text.clone(),
                        )],
                        ..Default::default()
                    },
                )
                .expect("configuration comes from admitted map bytes, not disk");
                let database =
                    db::Db::from_slang(&output.snapshot).expect("owned configured hierarchy");
                assert_eq!(
                    database.source_text("virtual/root.map"),
                    Some(text.as_str())
                );
                database
            };
            let design = model::DesignModel::from_db(&database);
            assert_eq!(design.instance("top.u").unwrap().def_name, expected);
            assert_eq!(design.instance("top.v").unwrap().def_name, "low");
        }
    }
}

#[test]
fn map_configuration_frontend_diagnostics_keep_original_lines_and_name() {
    use llg::core::compile;
    let error = compile::compile_sources_checked(
        &[compile::OwnedSource::compilation_unit(
            "virtual/top.sv",
            "module top; endmodule",
        )],
        &compile::CompileOpts {
            top: Some("broken:config".to_owned()),
            library_maps: vec![compile::OwnedSource::include(
                "virtual/broken.map",
                "// map header\n\nconfig broken;\n design ;\nendconfig\n",
            )],
            ..Default::default()
        },
    )
    .expect_err("Slang, not a map-token discard, must report missing design name");
    let diagnostics = error.diagnostics().expect("a frontend grammar diagnostic");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .file
            .as_deref()
            .is_some_and(|name| name.ends_with("virtual/broken.map"))
            && diagnostic.line == 4
            && diagnostic.message.contains("identifier")),
        "{diagnostics:?}"
    );
}

#[test]
fn packed_policy_matrix_keeps_frontend_runtime_and_lazy_arm_semantics() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            "continuation_12_15",
            "packed_conditional_matrix",
            "PACKED_POLICY_PASS\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn packed_policy_lowers_from_owned_sources_after_native_teardown() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        let database = {
            let output = compile::compile_sources_checked(
                &[compile::OwnedSource::compilation_unit(
                    "packed-policy.sv",
                    include_str!("fixtures/sim/continuation_12_15/packed_conditional_matrix.sv"),
                )],
                &compile::CompileOpts {
                    top: Some("tb".to_owned()),
                    edition,
                    ..Default::default()
                },
            )
            .expect("target-edition packed conditional source");
            db::Db::from_slang(&output.snapshot).expect("owned packed conditional source")
        };
        database.validate().unwrap();
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .expect("owned packed conditional lowering after snapshot destruction");
        }
    }
}

#[test]
fn structure_conditionals_keep_immediate_members_across_constant_runtime_and_nba_paths() {
    sim_cli::run_case_with_args(
        "continuation_12_15",
        "struct_conditional_matrix",
        "STRUCT_POLICY_PASS\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn structure_conditional_descriptors_and_lowering_survive_snapshot_teardown() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    let database = {
        let output = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "record-policy.sv",
                include_str!("fixtures/sim/continuation_12_15/struct_conditional_matrix.sv"),
            )],
            &compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .expect("fixed record conditional qualification source");
        db::Db::from_slang(&output.snapshot).expect("owned fixed record graph")
    };
    database.validate().unwrap();
    let mut records = 0;
    for id in database.node_ids() {
        if matches!(
            database.node_kind(id),
            db::NodeKind::Expr(db::ExprKind::Operation {
                op: db::Operation::Conditional,
                ..
            })
        ) && database.type_descriptor(id).is_some_and(|descriptor| {
            matches!(&descriptor.shape, db::TypeShape::Aggregate(layout)
                if layout.kind == db::AggregateKind::UnpackedStruct)
        }) {
            records += 1;
        }
    }
    assert!(
        records > 0,
        "a direct unpacked conditional must remain in owned source"
    );
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("direct and function-return record merging after snapshot drop");
    }
}

#[test]
fn type_keys_preserve_precedence_types_and_contexts_with_strict_negative_neighbors() {
    sim_cli::run_case_with_args(
        "continuation_12_15",
        "type_key_context_matrix",
        "TYPE_KEYS_PASS\n",
        "",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "continuation_12_15",
        "type_key_duplicate_index",
        "multiple keys for index",
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "continuation_12_15",
        "type_key_missing_coverage",
        "not all elements",
        &["--edition", "2009"],
    );
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "continuation_12_15",
            "type_key_incompatible_value",
            optimized,
            &["--edition", "2009"],
            &[],
            &[],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            stderr.contains("cannot be assigned to type")
                || stderr.contains("no implicit conversion from"),
            "expected a type-conversion diagnostic, not a generic rejection: {stderr}",
        );
    }
}

#[test]
fn full_type_key_matrix_lowers_without_borrowing_frontend_storage() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    let database = {
        let output = compile::compile_sources_checked(
            &[compile::OwnedSource::compilation_unit(
                "type-key-contexts.sv",
                include_str!("fixtures/sim/continuation_12_15/type_key_context_matrix.sv"),
            )],
            &compile::CompileOpts {
                top: Some("tb".to_owned()),
                ..Default::default()
            },
        )
        .expect("runtime-valued fixed type-key context matrix");
        db::Db::from_slang(&output.snapshot).expect("owned type-key context graph")
    };
    database.validate().unwrap();
    for options in [OptConfig::none(), OptConfig::default()] {
        codegen::generate_from_db_with_opts(&database, &options)
            .expect("type-key declaration, local, return, argument and NBA after snapshot drop");
    }
}

#[test]
fn array_valued_pattern_items_fill_their_subarray_in_order() {
    sim_cli::run_case_with_args(
        "continuation_12_15",
        "nested_row_patterns",
        "NESTED_ROW_PATTERNS_PASS\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn structure_parameters_and_mixed_equality_operands_are_values() {
    sim_cli::run_case_with_args(
        "continuation_12_15",
        "record_value_contexts",
        "RECORD_VALUE_CONTEXTS_PASS\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
