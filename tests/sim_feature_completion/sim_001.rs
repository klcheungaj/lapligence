//! SIM-001: region iteration and procedural time contracts.
//!
//! Outputs whose order the standard leaves open are checked as permitted
//! outcome sets or as partial orders derived from the IEEE 1800-2009 §4.5
//! reference algorithm, never as one chosen interprocess order.

use std::process::Output;

use super::sim_cli;

const SUITE: &str = "feature_completion/sim_001";

fn stdout_lines(label: &str, output: &Output) -> Vec<String> {
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// Check that `lines` is a permutation of `expected` in which every chain
/// appears in its listed relative order.
fn assert_partial_order(label: &str, lines: &[String], expected: &str, chains: &[&[&str]]) {
    let mut actual_sorted = lines.to_vec();
    actual_sorted.sort();
    let mut expected_sorted: Vec<String> = expected.lines().map(str::to_owned).collect();
    expected_sorted.sort();
    assert_eq!(
        actual_sorted, expected_sorted,
        "{label}: unexpected line multiset: {lines:#?}"
    );
    let position = |line: &str| {
        lines
            .iter()
            .position(|candidate| candidate == line)
            .unwrap_or_else(|| panic!("{label}: missing {line:?}"))
    };
    for chain in chains {
        for pair in chain.windows(2) {
            assert!(
                position(pair[0]) < position(pair[1]),
                "{label}: {:?} must precede {:?}: {lines:#?}",
                pair[0],
                pair[1]
            );
        }
    }
}

/// A runtime failure after some output: exact stdout, nonzero exit and the
/// named diagnostic.
fn assert_runtime_failure(label: &str, output: &Output, stdout: &str, diagnostic: &str) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{label}: unexpected success");
    assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
    assert!(stderr.contains(diagnostic), "{label}: {stderr}");
}

#[test]
fn region_litmus_follows_reference_algorithm_constraints() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_001/region_reentry.out");
    // One chain carries every ordering the algorithm fixes. "A woke a=1" is
    // an independent Active process: it may interleave with "A active a=1"
    // (§4.7) but must finish before the Inactive region runs.
    let chains: &[&[&str]] = &[
        &[
            "A active a=1",
            "A inactive a=1",
            "A inactive2 a=1",
            "A woke a=2",
            "R reactive b=1",
            "R re-inactive b=1",
            "R re-inactive2 b=1",
            "A from reactive b=2",
            "A inactive after reactive c=0",
            "A t=12 a=2 b=2 c=12",
            "R t=15 b=2 c=12",
        ],
        &["A woke a=1", "A inactive a=1"],
    ];
    sim_cli::run_case_checked_matrix(SUITE, "region_reentry", &[], &|label, output| {
        assert_partial_order(label, &stdout_lines(label, output), expected, chains);
    });
}

#[test]
fn both_region_sets_reenter_repeatedly_in_one_slot() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_001/region_pingpong.out");
    sim_cli::run_case(SUITE, "region_pingpong", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "region_pingpong", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "region_pingpong", expected);
}

#[test]
fn reactive_clock_edges_return_through_observed() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_001/observed_reentry.out");
    sim_cli::run_case(SUITE, "observed_reentry", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "observed_reentry", expected, &[], &[]);
}

#[test]
fn races_accept_only_permitted_outcomes() {
    // Each position lists every outcome the standard permits there.
    let allowed: &[&[&str]] = &[
        &["init x=9"],
        &["time0 w=10"],
        &["race a=1", "race a=2"],
        &["race b=0", "race b=5"],
        &["active c=0"],
        &["inactive c=0"],
        &["program sees d=3"],
        &["strobe c=7"],
        &["done b=5 c=7"],
    ];
    sim_cli::run_case_checked_matrix(SUITE, "race_outcomes", &[], &|label, output| {
        let lines = stdout_lines(label, output);
        assert_eq!(lines.len(), allowed.len(), "{label}: {lines:#?}");
        for (line, permitted) in lines.iter().zip(allowed) {
            assert!(
                permitted.contains(&line.as_str()),
                "{label}: {line:?} is not one of {permitted:?}"
            );
        }
    });
}

#[test]
fn every_time_unit_magnitude_schedules_one_local_unit() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_001/time_units.out");
    sim_cli::run_case(SUITE, "time_units", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "time_units", expected, &[], &[]);
}

#[test]
fn delays_round_at_local_precision_and_evaluate_each_execution() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_001/delay_rounding.out");
    sim_cli::run_case(SUITE, "delay_rounding", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "delay_rounding", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "delay_rounding", expected);
}

#[test]
fn negative_real_delays_become_unsigned_time() {
    let runtime =
        include_str!("../fixtures/sim/feature_completion/sim_001/negative_real_delay.out");
    sim_cli::run_case(SUITE, "negative_real_delay", runtime, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "negative_real_delay", runtime, &[], &[]);
    let constant =
        include_str!("../fixtures/sim/feature_completion/sim_001/negative_real_constant.out");
    sim_cli::run_case(SUITE, "negative_real_constant", constant, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "negative_real_constant", constant, &[], &[]);
}

#[test]
fn tick_range_failures_precede_queue_insertion() {
    let delay = include_str!("../fixtures/sim/feature_completion/sim_001/time_limit.out");
    sim_cli::run_case_checked_matrix(SUITE, "time_limit", &[], &|label, output| {
        assert_runtime_failure(
            label,
            output,
            delay,
            "simulation time overflow at 18446744073709551615 while scheduling a delay of 1 tick(s)",
        );
    });
    let nba = include_str!("../fixtures/sim/feature_completion/sim_001/time_limit_nba.out");
    sim_cli::run_case_checked_matrix(SUITE, "time_limit_nba", &[], &|label, output| {
        assert_runtime_failure(
            label,
            output,
            nba,
            "nonblocking assignment time or sequence overflow",
        );
    });
    let negative =
        include_str!("../fixtures/sim/feature_completion/sim_001/neg_negative_real_overflow.out");
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_negative_real_overflow",
        &[],
        &|label, output| {
            assert_runtime_failure(
                label,
                output,
                negative,
                "delay exceeds the 64-bit tick range",
            );
        },
    );
}

#[test]
fn nonfinite_and_illegal_delays_reject() {
    let stdout = include_str!("../fixtures/sim/feature_completion/sim_001/neg_nonfinite_delay.out");
    sim_cli::run_case_checked_matrix(SUITE, "neg_nonfinite_delay", &[], &|label, output| {
        assert_runtime_failure(label, output, stdout, "real delay must be finite");
    });
    sim_cli::reject_case(
        SUITE,
        "neg_function_delay",
        "statements that pass time are not allowed in this context",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_program_always",
        "member not allowed in program declaration",
    );
}
