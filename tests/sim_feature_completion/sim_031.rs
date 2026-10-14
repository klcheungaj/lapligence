//! SIM-031: `randcase` weighted procedural choice. Oracles come from IEEE
//! 1800-2009 18.16 as quoted in the fixture readme: deterministic choices are
//! exact, frequencies are bounded by a predeclared chi-square critical value,
//! and replay relations are compared across optimizer modes and value
//! backends instead of capturing generator-dependent numbers.
use std::cell::RefCell;
use std::process::Output;

use super::sim_cli;

const SUITE: &str = "feature_completion/sim_031";

/// Warnings the runtime issues for a randcase, one per execution.
fn randcase_warnings<'a>(stderr: &'a str, fixture: &str, what: &str) -> Vec<&'a str> {
    stderr
        .lines()
        .filter(|line| line.starts_with("llg: warning: randcase at ") && line.contains(what))
        .inspect(|line| {
            assert!(
                line.contains(&format!("{fixture}.sv:")),
                "warning must name its source: {line}"
            );
        })
        .collect()
}

// ── A01: only legal nonzero-weight branches execute ─────────────────────────

#[test]
fn a_single_nonzero_weight_is_always_chosen() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "single_weight",
        include_str!("../fixtures/sim/feature_completion/sim_031/single_weight.out"),
        &[],
        &[],
    );
}

#[test]
fn all_zero_weights_take_no_branch_and_warn_once_per_execution() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_031/all_zero.out");
    sim_cli::run_case_checked_matrix(SUITE, "all_zero", &[], &|label, output| {
        assert!(output.status.success(), "{label}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        // 3 task runs, 2 constant runs, 2 unknown runs, 1 masked weight.
        let warnings = randcase_warnings(&stderr, "all_zero", "all weights are zero");
        assert_eq!(warnings.len(), 8, "{label}: {stderr}");
    });
}

#[test]
fn weights_of_every_width_select_exactly() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "wide_weights",
        include_str!("../fixtures/sim/feature_completion/sim_031/wide_weights.out"),
        &[],
        &[],
    );
}

// ── A02: seed replay, frequency and side-effecting weights ──────────────────

/// Pearson chi-square against the weights with the p=0.001 critical value of
/// each set's degrees of freedom; seeds are fixed, so the result is
/// reproducible.
#[test]
fn weighted_frequencies_stay_inside_the_predeclared_chi_square_bound() {
    let mut expected = String::new();
    for seed in 1..=5 {
        for set in [
            "small 3:1:0:4",
            "skewed 1:99",
            "wide40 3:1",
            "wide100 3:1",
            "mixed 7:14",
        ] {
            expected.push_str(&format!("{set} seed {seed}: ok\n"));
        }
    }
    expected.push_str("failures=0\n");
    assert_eq!(
        expected,
        include_str!("../fixtures/sim/feature_completion/sim_031/frequency.out")
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "frequency",
        include_str!("../fixtures/sim/feature_completion/sim_031/frequency.out"),
        &[],
        &[],
    );
}

/// The selections depend on the generator, so the oracle is a relation: the
/// same seed replays, another seed differs, and every optimizer mode and value
/// backend draws the same checksum.
#[test]
fn a_seed_replays_the_same_selections_in_every_mode() {
    let outputs = RefCell::new(Vec::new());
    sim_cli::run_case_checked_matrix(SUITE, "seed_replay", &[], &|label, output: &Output| {
        assert!(output.status.success(), "{label}: {output:?}");
        outputs.borrow_mut().push((
            label.to_owned(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        ));
    });
    let outputs = outputs.into_inner();
    let (first_label, first) = &outputs[0];
    let relations = "replay equal: yes\nother seed differs: yes\nwide replay equal: yes\n";
    assert!(first.starts_with(relations), "{first_label}: {first}");
    assert!(
        first[relations.len()..].starts_with("checksum "),
        "{first_label}: {first}"
    );
    assert_eq!(outputs.len(), 6);
    for (label, output) in &outputs[1..] {
        assert_eq!(output, first, "{label} differs from {first_label}");
    }
}

/// Adding a thread after the existing ones, and drawing in it, leaves the
/// selections of the earlier threads and their forked children unchanged.
#[test]
fn selections_are_independent_of_other_threads() {
    let outputs = RefCell::new(Vec::new());
    for args in [&[][..], &["-D", "EXTRA_ACTIVITY"][..]] {
        sim_cli::run_case_checked_matrix(SUITE, "thread_stability", args, &|label, output| {
            assert!(output.status.success(), "{label}, {args:?}: {output:?}");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let kept = stdout
                .lines()
                .filter(|line| !line.starts_with("extra"))
                .map(|line| format!("{line}\n"))
                .collect::<String>();
            if args.is_empty() {
                assert!(!stdout.contains("extra"), "{label}: {stdout}");
            } else {
                assert!(stdout.contains("extra done\n"), "{label}: {stdout}");
            }
            outputs
                .borrow_mut()
                .push((format!("{label}, {args:?}"), kept));
        });
    }
    let outputs = outputs.into_inner();
    let (first_label, first) = &outputs[0];
    assert_eq!(first.lines().count(), 2, "{first_label}: {first}");
    assert!(first.starts_with("a ") && first.contains("\nb "), "{first}");
    for (label, output) in &outputs[1..] {
        assert_eq!(output, first, "{label} differs from {first_label}");
    }
}

#[test]
fn weights_are_evaluated_once_each_in_source_order() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "weight_side_effects",
        include_str!("../fixtures/sim/feature_completion/sim_031/weight_side_effects.out"),
        &[],
        &[],
    );
}

// ── A03: chosen bodies wait, fork, return and are disabled ──────────────────

#[test]
fn a_chosen_body_can_wait_fork_and_call_timed_tasks() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "body_wait_fork",
        include_str!("../fixtures/sim/feature_completion/sim_031/body_wait_fork.out"),
        &[],
        &[],
    );
}

#[test]
fn return_disable_break_and_continue_leave_a_chosen_body() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_031/body_control.out");
    sim_cli::run_case_checked_matrix(SUITE, "body_control", &[], &|label, output| {
        assert!(output.status.success(), "{label}: {output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert_eq!(stdout, expected, "{label}");
        // early(0) and task_return(0) have only zero weights.
        let stderr = String::from_utf8_lossy(&output.stderr);
        let warnings = randcase_warnings(&stderr, "body_control", "all weights are zero");
        assert_eq!(warnings.len(), 2, "{label}: {stderr}");
    });
}

// ── Negative cases ──────────────────────────────────────────────────────────

#[test]
fn a_real_weight_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_real_weight",
        "neg_real_weight.sv:7:7 expression type 'real' is not integral",
    );
}

#[test]
fn a_string_weight_is_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_string_weight",
        "neg_string_weight.sv:7:7 expression type 'string' is not integral",
    );
}

#[test]
fn a_weight_sum_beyond_the_packed_limit_is_a_source_addressed_error() {
    sim_cli::reject_case(
        SUITE,
        "neg_sum_overflow",
        "neg_sum_overflow.sv:8:5 in `tb`: the exact weight sum of 2 items with 1048575-bit \
         weights needs 1048576 bits, but packed values must be narrower than 1048576 bits",
    );
}
