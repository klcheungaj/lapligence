//! Public regressions for pruned-schedule positions four through seven.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn singleton_concat_preserves_unsigned_and_self_determined_semantics() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "n06_singleton_concat_signed_cast",
        "PASS n06_singleton_concat_signed_cast\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn singleton_concat_retains_legacy_unsigned_assignment() {
    for edition in ["2001", "2009"] {
        sim_cli::run_case_with_args(
            "review_bundle",
            "n06_singleton_concat_legacy",
            "PASS n06_singleton_concat_legacy\n",
            "",
            &[],
            &["--edition", edition],
        );
    }
}
