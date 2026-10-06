//! Public regressions for pruned-schedule positions eight through eleven.
use crate::sim_cli;

#[test]
fn tagged_pattern_comparisons_inherit_the_case_mode() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "n09_tagged_case_modes",
        "PASS n09_tagged_case_modes\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn whole_fixed_patterns_keep_types_snapshots_and_scopes() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "n08_whole_patterns",
        "PASS n08_whole_patterns\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "review_bundle",
        "n08_binding_scope_error",
        "undeclared identifier",
        &["--edition", "sv2009"],
    );
}

#[test]
fn mixed_recursive_assignment_patterns_preserve_contexts_and_precedence() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "n03_mixed_record_patterns",
        "PASS n03_mixed_record_patterns\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::run_case_with_args(
        "review_bundle",
        "n03_mixed_row_patterns",
        "PASS n03_mixed_row_patterns\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::run_case_with_args(
        "review_bundle",
        "n03_mixed_pattern_port",
        "PASS n03_mixed_pattern_port\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "review_bundle",
        "n03_mixed_duplicate_index",
        "multiple keys",
        &["--edition", "sv2009"],
    );
}

fn library_fixture(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/review_library_precedence")
        .join(name);
    assert!(
        path.is_file(),
        "missing library fixture: {}",
        path.display()
    );
    path.to_string_lossy().into_owned()
}

#[test]
fn library_specificity_and_duplicate_safe_mapping_work_in_both_editions() {
    for name in [
        "forward.map",
        "reversed.map",
        "resolved_tie.map",
        "same_library.map",
        "directory.map",
    ] {
        let map = library_fixture(name);
        for edition in ["v2001", "sv2009"] {
            sim_cli::run_case_with_source_prefix(
                "review_library_precedence",
                "top",
                &["config"],
                "mapped=22\n",
                "",
                &[],
                &["--edition", edition, "--top", "choose", "--libmap", &map],
            );
        }
    }
}

#[test]
fn explicit_library_assignment_overrides_overlapping_maps() {
    let map = library_fixture("override.map");
    let source = format!("chosen={}", library_fixture("rtl/cell.sv"));
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_source_prefix(
            "review_library_precedence",
            "top",
            &["config"],
            "mapped=22\n",
            "",
            &[],
            &[
                "--edition",
                edition,
                "--top",
                "choose",
                "--libmap",
                &map,
                "--libfile",
                &source,
            ],
        );
    }
}

#[test]
fn unresolved_winning_rank_library_ties_are_diagnosed() {
    let map = library_fixture("ambiguous.map");
    for edition in ["v2001", "sv2009"] {
        sim_cli::reject_case_with_args(
            "review_library_precedence",
            "top",
            "ambiguous library mapping",
            &["--edition", edition, "--top", "top", "--libmap", &map],
        );
    }
}

#[test]
fn exact_source_library_maps_preserve_configured_owned_hierarchy() {
    use llg::core::{compile, db, model};
    for edition in [
        compile::LanguageEdition::Verilog2001,
        compile::LanguageEdition::SystemVerilog2009,
    ] {
        for map in [
            "library A rtl/*.sv; library B rtl/*.sv; library chosen */cell.sv;",
            "library chosen */cell.sv; library A rtl/*.sv; library B rtl/*.sv;",
        ] {
            let database = {
                let output = compile::compile_sources_checked(
                    &[
                        compile::OwnedSource::compilation_unit(
                            "virtual/top.sv",
                            "module top; cell_body instance_name(); endmodule",
                        ),
                        compile::OwnedSource::compilation_unit(
                            "virtual/config.sv",
                            "config choose; design work.top; default liblist chosen; endconfig",
                        ),
                        compile::OwnedSource::compilation_unit(
                            "virtual/rtl/cell.sv",
                            "module cell_body; endmodule",
                        ),
                    ],
                    &compile::CompileOpts {
                        top: Some("choose:config".to_owned()),
                        edition,
                        library_maps: vec![compile::OwnedSource::include("virtual/root.map", map)],
                        ..Default::default()
                    },
                )
                .expect("configured in-memory library specificity");
                db::Db::from_slang(&output.snapshot).expect("owned mapped design")
            };
            let design = model::DesignModel::from_db(&database);
            let instance = design
                .instance("top.instance_name")
                .expect("configured child");
            assert_eq!(instance.def_name, "cell_body");
            assert_eq!(design.modules_in("virtual/rtl/cell.sv").len(), 1);
        }
    }
}
