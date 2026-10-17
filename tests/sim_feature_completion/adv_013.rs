//! ADV-013: multiclock concurrent assertions (IEEE 1800-2009 16.14, 16.17),
//! every clocking event form, `$global_clock` (14.14),
//! `$inferred_clock`/`$inferred_disable` (16.15.7) and recursive properties
//! (16.13.17, Annex F.7). Hand-derived fixtures compare their action lines
//! as a set (actions of different assertions in one time step have no
//! defined order); every positive case runs in both optimizer modes on the
//! legacy, compact/portable and compact/GMP value backends.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/adv_013";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_013/",
            $name
        ))
    };
}

fn sorted_lines(text: &str) -> Vec<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines.sort();
    lines
}

/// The run succeeds and its action lines are exactly `expected`, in any
/// order.
fn assert_line_set(label: &str, output: &Output, expected: &str) {
    assert!(
        output.status.success(),
        "{label}: {}",
        sim_cli::observed_output(output)
    );
    let stdout = llg::ffi::platform::native_text_to_lf(output.stdout.clone());
    let actual = sorted_lines(&String::from_utf8_lossy(&stdout));
    let expected = sorted_lines(expected);
    if actual != expected {
        let missing: Vec<_> = expected
            .iter()
            .filter(|line| actual.binary_search(line).is_err())
            .collect();
        let extra: Vec<_> = actual
            .iter()
            .filter(|line| expected.binary_search(line).is_err())
            .collect();
        panic!("{label}: missing {missing:?}; unexpected {extra:?}");
    }
}

/// A02 / 16.14.1-16.14.3: ##1 and ##0 between clocks, implications, clock
/// flow across |=> and parentheses, the adjointness examples and
/// juxtaposition, at coincident (30, 90) and separate ticks.
#[test]
fn multiclocked_sequences_and_clock_flow() {
    sim_cli::run_case_checked_matrix(SUITE, "flow", &[], &|label, output| {
        assert_line_set(label, output, fixture!("flow.out"))
    });
}

/// A02 / 16.5, 14.13, 14.14, 16.15.7: value-change, `edge`, `or`-list,
/// clocking-block, bit-select, gated and default event-list clocks tick once
/// per time step; `$global_clock` clocks assertions and event controls;
/// inferred clock and disable defaults resolve at the instance.
#[test]
fn clocking_event_forms_and_clock_resolution_functions() {
    sim_cli::run_case_checked_matrix(SUITE, "clocks", &[], &|label, output| {
        assert_line_set(label, output, fixture!("clocks.out"))
    });
}

/// A02 negatives: clause-linked rejections of illegal multiclock forms and
/// clock resolution.
#[test]
fn illegal_multiclock_forms_are_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_mc_repetition",
        "sequence operator repetition combines differently clocked or multiclocked sequence \
         operands; only ##1 and ##0 may (IEEE 1800-2009 16.14.1)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_mc_first_match",
        "sequence operator first_match combines differently clocked or multiclocked sequence \
         operands; only ##1 and ##0 may (IEEE 1800-2009 16.14.1)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_mc_delay",
        "neg_mc_delay.sv:5:41 multiclocked sequence operands cannot be combined with any \
         sequence operators other than ##1 and ##0",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_mc_empty",
        "neg_mc_empty.sv:5:43 sequence admits an empty match; this is not allowed when \
         concatenating differently clocked sequences",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_leading_clocks",
        "neg_leading_clocks.sv:5:23 assertion expression does not have a unique leading clock",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_inferred_no_clock",
        "neg_inferred_no_clock.sv:6:6 no inferred clock in this context",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_global_clock_missing",
        "neg_global_clock_missing.sv:5:25 no global clocking is available in this scope",
    );
}
