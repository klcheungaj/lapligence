use super::run_case_with_stderr;

#[test]
fn immediate_assertions_use_four_state_truth_and_evaluate_once() {
    run_case_with_stderr(
        "assertions",
        "assert pass\nassert fail\nassume fail\ncover pass\nevals=5\n",
        concat!(
            "llg: $finish at time 0 at tb:21:5\n",
            "llg: simulation statistics: processes=1\n",
            "llg: assertion counts: assert_failed=0 assume_failed=0 cover=1\n",
        ),
    );
}

#[test]
fn omitted_failure_actions_report_assert_and_assume_but_not_cover() {
    run_case_with_stderr(
        "assertions_default",
        "",
        concat!(
            "llg: assertion assert failed: tb:4:5 (named_assert)\n",
            "llg: assertion assume failed: tb:5:5\n",
            "llg: $finish at time 0 at tb:7:5\n",
            "llg: simulation statistics: processes=1\n",
            "llg: severity counts: info=0 warning=0 error=2 fatal=0\n",
            "llg: assertion counts: assert_failed=1 assume_failed=1 cover=0\n",
        ),
    );
}

#[test]
fn deferred_assertion_captures_values_and_reads_refs_in_reactive() {
    run_case_with_stderr(
        "deferred_assertions",
        "sampled=0 reference=1\n",
        concat!(
            "llg: $finish at time 1000 at tb:14:5\n",
            "llg: simulation statistics: processes=1\n",
        ),
    );
}

#[test]
fn deferred_assertions_report_defaults_and_cover_in_reactive() {
    run_case_with_stderr(
        "deferred_assertions_default",
        "",
        concat!(
            "llg: assertion assert failed: tb:4:5\n",
            "llg: assertion assume failed: tb:5:5\n",
            "llg: $finish at time 1000 at tb:10:5\n",
            "llg: simulation statistics: processes=1\n",
            "llg: severity counts: info=0 warning=0 error=2 fatal=0\n",
            "llg: assertion counts: assert_failed=1 assume_failed=1 cover=1\n",
        ),
    );
}

/// SV 16.4.1/16.4.2: each execution queues a report and only a flush point
/// clears the queue, so the loop's failing execution still reports; `#0` is
/// no flush point, and the report matures in the Observed region of time 0.
#[test]
fn deferred_assertions_queue_each_execution_until_a_flush_point() {
    run_case_with_stderr(
        "deferred_assertions_glitch",
        "reports=0\nglitch=0\nreports=1\n",
        concat!(
            "llg: $finish at time 1000 at tb:17:5\n",
            "llg: simulation statistics: processes=1\n",
        ),
    );
}

#[test]
fn deferred_assertions_support_module_level_actions() {
    run_case_with_stderr(
        "deferred_assertions_module",
        "module=1\n",
        concat!(
            "llg: $finish at time 1000 at tb:13:5\n",
            "llg: simulation statistics: processes=2\n",
        ),
    );
}

/// A report still pending when `$finish` runs is dropped; the final
/// procedure runs next (S36-D6).
#[test]
fn deferred_assertions_pending_at_finish_are_dropped() {
    run_case_with_stderr(
        "deferred_assertions_finish",
        "final-after\n",
        concat!(
            "llg: $finish at time 0 at tb:5:5\n",
            "llg: simulation statistics: processes=1\n",
        ),
    );
}

#[test]
fn deferred_assertions_reject_multi_statement_actions() {
    super::reject_case(
        "deferred_assertions_multistmt_rejected",
        "deferred assertion action must be a subroutine call",
    );
}

#[test]
fn deferred_assertions_reject_automatic_reference_actions() {
    super::reject_case(
        "deferred_assertions_ref_rejected",
        "deferred assertion action",
    );
}

#[test]
fn deferred_assertions_reject_timing_actions() {
    super::reject_case(
        "deferred_assertions_timing_rejected",
        "cannot contain timing controls",
    );
}
