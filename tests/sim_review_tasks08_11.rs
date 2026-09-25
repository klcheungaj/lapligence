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

#[test]
fn whole_fixed_patterns_keep_types_snapshots_and_scopes() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "n08_whole_patterns",
        "PASS n08_whole_patterns\n",
        "",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        "review_bundle",
        "n08_binding_scope_error",
        "undeclared identifier",
        &["--edition", "2009"],
    );
}
