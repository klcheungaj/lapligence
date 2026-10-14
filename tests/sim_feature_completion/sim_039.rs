//! SIM-039: assertion local formals, match items, sequence endpoints and the
//! 2009 assertion action controls (IEEE 1800-2009 §§16.8, 16.10-16.11,
//! 16.14.5-16.14.6, 9.4.2.4, 9.4.4, 20.11-20.12). Every expected output is
//! derived by hand in the fixture readme.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_039";

fn sorted_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

/// Same-time action order is not specified by the LRM, so lines compare as
/// sets; every line carries its time.
fn assert_sorted_stdout(label: &str, output: &Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        sorted_lines(&String::from_utf8_lossy(&output.stdout)),
        sorted_lines(expected),
        "{label}: lines differ from the hand-derived set"
    );
}

fn stderr_lines<'o>(stderr: &'o str, prefix: &str) -> Vec<&'o str> {
    stderr
        .lines()
        .filter(|line| line.starts_with(prefix))
        .collect()
}

#[test]
fn action_controls_select_scopes_and_leave_counters_unchanged() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_039/action_controls.out");
    sim_cli::run_case_checked_matrix(SUITE, "action_controls", &[], &|label, output| {
        assert_sorted_stdout(label, output, expected);
        let stderr = String::from_utf8_lossy(&output.stderr);
        // `nd` fails at 35 (reported) and 65 (fail action off, not reported).
        let failures = stderr_lines(&stderr, "llg: assertion assert failed: ");
        assert_eq!(failures.len(), 1, "{label}: {stderr}");
        assert!(failures[0].ends_with("(nd)"), "{label}: {stderr}");
        // Counters: 8 failures (u1, u2, top_chk, nd at 35 and 65), 3 cover
        // hits (25, 45, 75) and 16 vacuous successes (4 assertions at 5, 15,
        // 55, 85), whichever actions were suppressed.
        assert!(
            stderr.contains("llg: assertion counts: assert_failed=8 assume_failed=0 cover=3\n"),
            "{label}: {stderr}"
        );
        assert!(
            stderr.contains("llg: assertion vacuous=16\n"),
            "{label}: {stderr}"
        );
    });
}

#[test]
fn action_controls_gate_immediate_deferred_and_expect_actions() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_039/action_procedural.out");
    sim_cli::run_case_checked_matrix(SUITE, "action_procedural", &[], &|label, output| {
        assert_sorted_stdout(label, output, expected);
        let stderr = String::from_utf8_lossy(&output.stderr);
        // `nde` reports at 0 and 1; at 4 and 5 its fail action is off.
        let failures = stderr_lines(&stderr, "llg: assertion assert failed: ");
        assert_eq!(failures.len(), 2, "{label}: {stderr}");
        assert!(
            stderr.contains(
                "llg: assertion counts: assert_failed=4 assume_failed=0 expect_failed=1 cover=0\n"
            ),
            "{label}: {stderr}"
        );
    });
}

#[test]
fn control_selectors_name_generate_blocks_named_blocks_and_subroutines() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_039/selectors.out");
    sim_cli::run_case_checked_matrix(SUITE, "selectors", &[], &|label, output| {
        assert_sorted_stdout(label, output, expected)
    });
}
