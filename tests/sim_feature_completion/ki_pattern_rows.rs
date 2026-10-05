use super::sim_cli;

const SUITE: &str = "feature_completion/ki_pattern_rows";

#[test]
fn positional_patterns_scatter_small_sources_into_rows() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_pattern_rows/dense_rows.out");
    sim_cli::run_case(SUITE, "dense_rows", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "dense_rows", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "dense_rows", expected);
}

#[test]
fn positional_patterns_copy_descriptor_rows_without_flattening() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_pattern_rows/descriptor_rows.out");
    sim_cli::run_case(SUITE, "descriptor_rows", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_rows", expected, &[], &[]);
}

#[test]
fn continuous_positional_patterns_drive_rows() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_pattern_rows/continuous_rows.out");
    sim_cli::run_case(SUITE, "continuous_rows", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "continuous_rows", expected, &[], &[]);
}

#[test]
fn neg_row_pattern_forms() {
    sim_cli::reject_case(
        SUITE,
        "neg_row_shape",
        "value of type 'logic[7:0]$[4]' cannot be assigned to type 'logic[7:0]$[3]'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_row_runtime_continuous",
        "continuous assignment-pattern LHS in `tb` requires constant select indices",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_row_automatic_nba",
        "nonblocking assignment to an automatic assignment-pattern target in `tb` is not supported",
    );
}
