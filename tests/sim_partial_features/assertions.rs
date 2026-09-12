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
