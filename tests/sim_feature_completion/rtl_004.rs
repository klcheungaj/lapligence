use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_004";

#[test]
fn packed_vector_and_row_type_keys() {
    sim_cli::run_case(
        SUITE,
        "vector_keys",
        include_str!("../fixtures/sim/feature_completion/rtl_004/vector_keys.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "vector_keys",
        include_str!("../fixtures/sim/feature_completion/rtl_004/vector_keys.out"),
    );
}

#[test]
fn nested_rows_and_array_valued_items() {
    sim_cli::run_case(
        SUITE,
        "nested_rows",
        include_str!("../fixtures/sim/feature_completion/rtl_004/nested_rows.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "nested_rows",
        include_str!("../fixtures/sim/feature_completion/rtl_004/nested_rows.out"),
    );
}

#[test]
fn scatter_overlap_and_selector_capture() {
    sim_cli::run_case(
        SUITE,
        "scatter_capture",
        include_str!("../fixtures/sim/feature_completion/rtl_004/scatter_capture.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "scatter_capture",
        include_str!("../fixtures/sim/feature_completion/rtl_004/scatter_capture.out"),
    );
}

#[test]
fn mixed_state_records_and_last_type_key() {
    sim_cli::run_case(
        SUITE,
        "record_keys",
        include_str!("../fixtures/sim/feature_completion/rtl_004/record_keys.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "record_keys",
        include_str!("../fixtures/sim/feature_completion/rtl_004/record_keys.out"),
    );
}

#[test]
fn side_effect_values_without_multiplicity_oracle() {
    sim_cli::run_case(
        SUITE,
        "effect_values",
        include_str!("../fixtures/sim/feature_completion/rtl_004/effect_values.out"),
        "",
        &[],
    );
}

#[test]
fn descriptor_patterns_preserve_sparse_defaults_and_nba() {
    sim_cli::run_case(
        SUITE,
        "descriptor_patterns",
        include_str!("../fixtures/sim/feature_completion/rtl_004/descriptor_patterns.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "descriptor_patterns",
        include_str!("../fixtures/sim/feature_completion/rtl_004/descriptor_patterns.out"),
    );
}

#[test]
fn neg_duplicate_pattern_index() {
    sim_cli::reject_case(SUITE, "neg_duplicate_pattern_index", "duplicate");
}

#[test]
fn neg_keyed_pattern_lvalue() {
    sim_cli::reject_case(SUITE, "neg_keyed_pattern_lvalue", "not assignable");
}

#[test]
fn neg_replicated_pattern_lvalue() {
    sim_cli::reject_case(SUITE, "neg_replicated_pattern_lvalue", "not assignable");
}

#[test]
fn neg_automatic_nba() {
    sim_cli::reject_case(SUITE, "neg_automatic_nba", "automatic");
}

#[test]
fn neg_uncovered() {
    sim_cli::reject_case(SUITE, "neg_uncovered", "not all elements");
}

#[test]
fn neg_shape() {
    sim_cli::reject_case(SUITE, "neg_shape", "requires 3 elements");
}

#[test]
fn neg_default_lvalue() {
    sim_cli::reject_case(SUITE, "neg_default_lvalue", "not assignable");
}

#[test]
fn neg_ref_nba() {
    // IEEE 1800-2009 13.5.2: the static task's ref formal rejects first.
    sim_cli::reject_case(
        SUITE,
        "neg_ref_nba",
        "'ref' arguments can only be used in 'automatic' subroutines",
    );
}

#[test]
fn neg_constant_lvalue() {
    sim_cli::reject_case(SUITE, "neg_constant_lvalue", "assignable");
}

#[test]
fn scatter_records_overlap_and_persistent_nba() {
    sim_cli::run_case(
        SUITE,
        "scatter_records",
        include_str!("../fixtures/sim/feature_completion/rtl_004/scatter_records.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "scatter_records",
        include_str!("../fixtures/sim/feature_completion/rtl_004/scatter_records.out"),
    );
}

#[test]
fn adopted_net_pattern_driver() {
    sim_cli::run_case(
        SUITE,
        "net_pattern_driver",
        include_str!("../fixtures/sim/feature_completion/rtl_004/net_pattern_driver.out"),
        "",
        &[],
    );
}

#[test]
fn neg_negative_replication() {
    sim_cli::reject_case(SUITE, "neg_negative_replication", "replication");
}

#[test]
fn neg_automatic_scatter_nba() {
    sim_cli::reject_case(SUITE, "neg_automatic_scatter_nba", "automatic");
}

#[test]
fn descriptor_pattern_source_stays_bounded() {
    use super::sim_harness;
    use std::process::Command;
    use std::time::Duration;
    let mut sizes = Vec::new();
    for count in [65537, 16777216] {
        let directory = sim_harness::TempDir::new("rtl004-source-shape").expect("scratch");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/sim/feature_completion/rtl_004/descriptor_scale.sv");
        let output = sim_harness::run_command(
            Command::new(env!("CARGO_BIN_EXE_llg"))
                .arg(source)
                .args(["--gen-only", "-D"])
                .arg(format!("PATTERN_CELLS={count}"))
                .arg("--out-dir")
                .arg(directory.path()),
            Duration::from_secs(60),
        )
        .expect("generate pattern model");
        assert!(output.status.success(), "{output:?}");
        let source =
            std::fs::read_to_string(directory.path().join("sim/tb/model.c")).expect("model source");
        assert!(
            source.len() < 100_000,
            "unexpected model size: {}",
            source.len()
        );
        sizes.push(source.len());
    }
    assert!(sizes[0].abs_diff(sizes[1]) < 1024, "{sizes:?}");
    sim_cli::run_case(
        SUITE,
        "descriptor_scale",
        include_str!("../fixtures/sim/feature_completion/rtl_004/descriptor_scale.out"),
        "",
        &[],
    );
    sim_cli::run_case_with_args(
        SUITE,
        "descriptor_scale",
        include_str!("../fixtures/sim/feature_completion/rtl_004/descriptor_scale.out"),
        "",
        &[],
        &["-D", "PATTERN_CELLS=16777216"],
    );
}
