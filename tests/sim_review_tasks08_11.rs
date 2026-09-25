//! Public regressions for pruned-schedule positions eight through eleven.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn tagged_pattern_comparisons_inherit_the_case_mode() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "n09_tagged_case_modes",
        "PASS n09_tagged_case_modes\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}
