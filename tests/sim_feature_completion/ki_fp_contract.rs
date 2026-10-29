use super::sim_cli;

const SUITE: &str = "feature_completion/ki_fp_contract";

#[test]
fn real_operators_round_separately_public_cli() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "real_operators_round_separately",
        include_str!(
            "../fixtures/sim/feature_completion/ki_fp_contract/real_operators_round_separately.out"
        ),
        &[],
        &[],
    );
}
