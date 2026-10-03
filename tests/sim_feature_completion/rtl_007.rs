use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_007";

#[test]
fn fixed_aggregates_cross_every_formal_direction() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/formal_directions.out");
    sim_cli::run_case(SUITE, "formal_directions", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "formal_directions", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "formal_directions", expected);
}

#[test]
fn references_forward_through_nested_calls_and_selected_actuals() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/nested_forwarding.out");
    sim_cli::run_case(SUITE, "nested_forwarding", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "nested_forwarding", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "nested_forwarding", expected);
}

#[test]
fn named_and_default_aggregate_arguments_evaluate_defaults_once() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/named_defaults.out");
    sim_cli::run_case(SUITE, "named_defaults", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "named_defaults", expected, &[], &[]);
}

#[test]
fn early_return_and_local_disable_keep_aggregate_results() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/early_exits.out");
    sim_cli::run_case(SUITE, "early_exits", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "early_exits", expected, &[], &[]);
}

#[test]
fn package_interface_and_module_subroutines_keep_their_environment() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/ownership.out");
    sim_cli::run_case(SUITE, "ownership", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "ownership", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "ownership", expected);
}

#[test]
fn oversized_arrays_cross_calls_as_descriptors() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/descriptor_calls.out");
    sim_cli::run_case(SUITE, "descriptor_calls", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_calls", expected, &[], &[]);
}

#[test]
fn evaluators_admit_helpers_with_private_loops_and_locals() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/evaluator_helpers.out");
    sim_cli::run_case(
        SUITE,
        "evaluator_helpers",
        expected,
        "",
        &["repeat count in `tb` is not a constant; evaluated at runtime"],
    );
    sim_cli::run_case_backend_parity(SUITE, "evaluator_helpers", expected, &[], &[]);
}

#[test]
fn stateful_helpers_are_evaluated_by_the_waiting_process() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/stateful_helpers.out");
    sim_cli::run_case(SUITE, "stateful_helpers", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "stateful_helpers", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "stateful_helpers", expected);
}

#[test]
fn process_evaluated_waits_detach_on_disable() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/stateful_cancel.out");
    sim_cli::run_case(SUITE, "stateful_cancel", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "stateful_cancel", expected, &[], &[]);
}

#[test]
fn oversized_arrays_reach_evaluator_helpers_by_descriptor() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_007/descriptor_evaluators.out");
    sim_cli::run_case(SUITE, "descriptor_evaluators", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "descriptor_evaluators", expected, &[], &[]);
}

#[test]
fn unused_results_keep_observable_calls() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_007/unused_results.out");
    sim_cli::run_case(SUITE, "unused_results", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "unused_results", expected, &[], &[]);
}

#[test]
fn stateful_event_helper_witness() {
    // Adopted FND-002 witness for L-F09-06-01 (SV2009 9.4.2, 13.4).
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_007/stateful_event_witness.out");
    sim_cli::run_case_with_args(
        SUITE,
        "stateful_event_witness",
        expected,
        "llg: $finish at time 1000 at tb:7:1\n",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "stateful_event_witness",
        expected,
        &["--edition", "2009"],
        &[],
    );
}

#[test]
fn real_readonly_helper_witness() {
    // Adopted FND-002 witness for L-F09-06-01 (SV2009 13.4, 10.3, 6.12).
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_007/real_readonly_witness.out");
    sim_cli::run_case_with_args(
        SUITE,
        "real_readonly_witness",
        expected,
        "llg: $finish at time 1000 at tb:7:1\n",
        &[],
        &["--edition", "2009"],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "real_readonly_witness",
        expected,
        &["--edition", "2009"],
        &[],
    );
}

#[test]
fn neg_actual_shape() {
    sim_cli::reject_case(
        SUITE,
        "neg_actual_shape",
        "cannot be assigned to type 'arr_t'",
    );
}

#[test]
fn neg_output_expression() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_expression",
        "expression is not assignable",
    );
}

#[test]
fn neg_temporary_ref() {
    sim_cli::reject_case(
        SUITE,
        "neg_temporary_ref",
        "invalid expression for pass by reference",
    );
}

#[test]
fn neg_const_ref_temporary() {
    sim_cli::reject_case(
        SUITE,
        "neg_const_ref_temporary",
        "invalid expression for pass by reference",
    );
}

#[test]
fn neg_ref_type() {
    sim_cli::reject_case(
        SUITE,
        "neg_ref_type",
        "connects to 'ref' port of inequivalent type 'r1_t'",
    );
}

#[test]
fn neg_function_delay() {
    sim_cli::reject_case(
        SUITE,
        "neg_function_delay",
        "statements that pass time are not allowed in this context",
    );
}

#[test]
fn neg_task_in_function() {
    sim_cli::reject_case(
        SUITE,
        "neg_task_in_function",
        "cannot invoke a task from within a function",
    );
}

#[test]
fn neg_continuous_output() {
    sim_cli::reject_case(
        SUITE,
        "neg_continuous_output",
        "cannot call a function with output, inout, or ref arguments outside of a procedural context",
    );
}

#[test]
fn neg_event_ref_formal() {
    sim_cli::reject_case(
        SUITE,
        "neg_event_ref_formal",
        "cannot call a function with output, inout, or ref arguments from an event expression",
    );
}

#[test]
fn neg_continuous_called_conflict() {
    sim_cli::reject_case(
        SUITE,
        "neg_continuous_called_conflict",
        "has both a continuous assignment",
    );
}

#[test]
fn neg_force_stateful() {
    // Unsupported boundary, not a language rule: force evaluators stay
    // read-only runtime callbacks.
    sim_cli::reject_case(
        SUITE,
        "neg_force_stateful",
        "is not a read-only evaluator: function body writes external or persistent storage",
    );
}

#[test]
fn neg_named_event_mix() {
    // Unsupported boundary, not a language rule.
    sim_cli::reject_case(
        SUITE,
        "neg_named_event_mix",
        "named events cannot share an event control with a process-evaluated helper expression",
    );
}
