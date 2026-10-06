use super::sim_cli;

const SUITE: &str = "feature_completion/fnd_003";
const EXPECTED: &str =
    include_str!("../fixtures/sim/feature_completion/fnd_003/packed_pattern.out");

#[test]
fn packed_pattern_public_cli() {
    for level in ["O0", "O3"] {
        sim_cli::run_case_with_args(
            SUITE,
            "packed_pattern",
            EXPECTED,
            "",
            &[],
            &["--edition", "sv2009", "--model-opt-level", level],
        );
    }
}

#[test]
fn packed_pattern_owned_execution_after_db_drop() {
    sim_cli::run_case_after_db_drop(SUITE, "packed_pattern", EXPECTED);
}
