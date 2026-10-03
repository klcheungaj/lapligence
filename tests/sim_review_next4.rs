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
fn abrupt_loop_exits_keep_a_stateful_static_result_process_evaluated() {
    // The jump bypasses the result write, so the persistent value is
    // retained after `toggle` returns to zero: one detected change.
    for fixture in ["n02_static_do_break_event", "n02_static_do_continue_event"] {
        sim_cli::run_case_with_args(
            "review_bundle",
            fixture,
            "PERSISTENT_RESULT changes=1\n",
            "",
            &[],
            &["--edition", "2009"],
        );
    }
}

#[test]
fn ordinary_static_state_and_proven_callback_results_remain_distinct() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "n02_static_do_state_control",
        "PASS n02_static_do_state_control\n",
        "",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::run_case_with_args(
        "review_bundle",
        "n02_static_definite_event",
        "PASS n02_static_definite_event\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_ordering_captures_automatic_and_formal_receivers_once() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "n07_selected_automatic_ordering",
        "PASS n07_selected_automatic_ordering\n",
        "",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::run_case_with_args(
        "review_bundle",
        "n07_selected_formal_ordering",
        "PASS n07_selected_formal_ordering\n",
        "",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::run_case_with_args(
        "review_bundle",
        "n07_selected_record_and_row_ordering",
        "PASS n07_selected_record_and_row_ordering\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn fixed_ordering_does_not_make_a_const_ref_receiver_writable() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "review_bundle",
            "n07_const_ref_receiver",
            optimized,
            &["--edition", "2009"],
            &[],
            &[],
        );
        assert!(!output.status.success(), "const ref receiver was writable");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("fixed value is not writable")
                || stderr.contains("cannot write through const ref")
                || stderr.contains("constant 'matrix' is not modifiable"),
            "unexpected const-ref diagnostic: {stderr}"
        );
    }
}
