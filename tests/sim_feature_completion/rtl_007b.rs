use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_007b";

#[test]
fn effectful_force_sources_follow_operands_while_in_effect() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007b/force_sources.out");
    sim_cli::run_case(SUITE, "force_sources", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "force_sources", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "force_sources", expected);
}

#[test]
fn effectful_force_sources_stop_on_release_and_replacement() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007b/force_lifecycle.out");
    sim_cli::run_case(SUITE, "force_lifecycle", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "force_lifecycle", expected, &[], &[]);
}

#[test]
fn intra_assignment_and_nonblocking_controls_evaluate_effectful_helpers() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_007b/intra_assignments.out");
    sim_cli::run_case(SUITE, "intra_assignments", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "intra_assignments", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "intra_assignments", expected);
}

#[test]
fn named_event_and_real_lists_mix_with_effectful_helpers() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007b/mixed_events.out");
    sim_cli::run_case(SUITE, "mixed_events", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "mixed_events", expected, &[], &[]);
}

#[test]
fn postponed_arguments_keep_private_helper_state() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_007b/postponed_helpers.out");
    sim_cli::run_case(SUITE, "postponed_helpers", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "postponed_helpers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "postponed_helpers", expected);
}

#[test]
fn neg_postponed_visible_write() {
    sim_cli::reject_case(
        SUITE,
        "neg_postponed_visible_write",
        "arguments are evaluated in the read-only Postponed region (IEEE 1800-2009 4.4.2.9)",
    );
}

#[test]
fn neg_nba_automatic_event() {
    sim_cli::reject_case(
        SUITE,
        "neg_nba_automatic_event",
        "cannot refer to automatic variable 'i' from intra-assignment event control of nonblocking assignment",
    );
}

#[test]
fn neg_named_event_array() {
    // Unsupported boundary, not a language rule.
    sim_cli::reject_case(
        SUITE,
        "neg_named_event_array",
        "a named event cannot share an event control with a process-evaluated helper that reads array, container or string storage",
    );
}
