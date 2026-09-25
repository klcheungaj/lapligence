//! Continuation coverage for schedule positions twelve through fifteen.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

fn map_fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/review_library_configs").join(name);
    assert!(path.is_file(), "missing map fixture: {}", path.display());
    path.to_string_lossy().into_owned()
}

#[test]
fn embedded_map_configurations_match_separate_sources_in_both_editions() {
    for edition in ["2001", "2009"] {
        for name in ["root.map", "included.map", "multiple.map"] {
            let map = map_fixture(name);
            sim_cli::run_case_with_args(
                "review_library_configs", "top", "mapped=22 11\n", "", &[],
                &["--edition", edition, "--top", "choose:config", "--libmap", &map],
            );
        }
        let map = map_fixture("separate.map");
        sim_cli::run_case_with_source_prefix(
            "review_library_configs", "top", &["config"], "mapped=22 11\n", "", &[],
            &["--edition", edition, "--top", "choose:config", "--libmap", &map],
        );
        let map = map_fixture("unterminated.map");
        sim_cli::reject_case_with_args(
            "review_library_configs", "top", "missing endconfig",
            &["--edition", edition, "--top", "choose:config", "--libmap", &map],
        );
    }
}

#[test]
fn logical_map_configuration_changes_reach_owned_hierarchy_after_snapshot_teardown() {
    use llg::core::{compile, db, model};
    for edition in [compile::LanguageEdition::Verilog2001, compile::LanguageEdition::SystemVerilog2009] {
        for (first, expected) in [("high", "high"), ("low", "low")] {
            let text = format!(
                "library cells rtl/cells.sv;\nconfig choose; design custom.top; \
                 cell selected_leaf use cells.low; instance top.u use cells.{first}; endconfig\n"
            );
            let database = {
                let output = compile::compile_sources_checked(
                    &[
                        compile::OwnedSource::compilation_unit("virtual/top.sv",
                            "module top; selected_leaf u(); selected_leaf v(); endmodule"),
                        compile::OwnedSource::include("virtual/rtl/cells.sv",
                            "module low; endmodule module high; endmodule"),
                    ],
                    &compile::CompileOpts {
                        top: Some("choose:config".to_owned()), edition,
                        default_library: Some("custom".to_owned()),
                        library_maps: vec![compile::OwnedSource::include("virtual/root.map", text.clone())],
                        ..Default::default()
                    },
                ).expect("configuration comes from admitted map bytes, not disk");
                let database = db::Db::from_slang(&output.snapshot).expect("owned configured hierarchy");
                assert_eq!(database.source_text("virtual/root.map"), Some(text.as_str()));
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
        &[compile::OwnedSource::compilation_unit("virtual/top.sv", "module top; endmodule")],
        &compile::CompileOpts {
            top: Some("broken:config".to_owned()),
            library_maps: vec![compile::OwnedSource::include(
                "virtual/broken.map", "// map header\n\nconfig broken;\n design ;\nendconfig\n",
            )],
            ..Default::default()
        },
    ).expect_err("Slang, not a map-token discard, must report missing design name");
    let diagnostics = error.diagnostics().expect("a frontend grammar diagnostic");
    assert!(diagnostics.iter().any(|diagnostic|
        diagnostic.file.as_deref().is_some_and(|name| name.ends_with("virtual/broken.map"))
            && diagnostic.line == 4
            && diagnostic.message.contains("identifier")
    ), "{diagnostics:?}");
}

#[test]
fn packed_policy_matrix_keeps_frontend_runtime_and_lazy_arm_semantics() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            "continuation_12_15", "packed_conditional_matrix", "PACKED_POLICY_PASS\n", "", &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn packed_policy_lowers_from_owned_sources_after_native_teardown() {
    use llg::core::{compile, db};
    use llg::sim::{codegen, opt::OptConfig};
    for edition in [compile::LanguageEdition::Verilog2001, compile::LanguageEdition::SystemVerilog2009] {
        let database = {
            let output = compile::compile_sources_checked(
                &[compile::OwnedSource::compilation_unit("packed-policy.sv",
                    include_str!("fixtures/sim/continuation_12_15/packed_conditional_matrix.sv"))],
                &compile::CompileOpts { top: Some("tb".to_owned()), edition, ..Default::default() },
            ).expect("target-edition packed conditional source");
            db::Db::from_slang(&output.snapshot).expect("owned packed conditional source")
        };
        database.validate().unwrap();
        for options in [OptConfig::none(), OptConfig::default()] {
            codegen::generate_from_db_with_opts(&database, &options)
                .expect("owned packed conditional lowering after snapshot destruction");
        }
    }
}
