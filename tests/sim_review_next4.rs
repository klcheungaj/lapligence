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

#[test]
fn tagged_member_reads_preserve_runtime_signedness() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "n01_tagged_signed_widen",
        "PASS n01_tagged_signed_widen\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn abrupt_loop_exits_cannot_hide_a_stateful_static_callback() {
    for fixture in ["n02_static_do_break_event", "n02_static_do_continue_event"] {
        sim_cli::reject_case_with_args(
            "review_bundle",
            fixture,
            "static function return is read or is not assigned on every path",
            &["--edition", "2009"],
        );
    }
}

#[test]
fn ordinary_static_state_and_proven_callback_results_remain_distinct() {
    for fixture in ["n02_static_do_state_control", "n02_static_definite_event"] {
        sim_cli::run_case_with_args(
            "review_bundle",
            fixture,
            &format!("PASS {fixture}\n"),
            "",
            &[],
            &["--edition", "2009"],
        );
    }
}
