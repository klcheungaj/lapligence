use super::run_case;

#[test]
fn automatic_event_evaluators_keep_nested_task_activations_separate() {
    run_case("event_activation_capture", "task=1\nwakes=1\n");
}

#[test]
fn pure_input_and_const_ref_functions_use_event_expression_dependencies() {
    run_case("event_pure_functions", "changes=3 rises=1 const=2\n");
}

#[test]
fn input_function_calls_keep_dynamic_array_dependencies() {
    run_case("event_function_array", "array_changes=2\n");
}

#[test]
fn function_event_values_preserve_four_state_edges() {
    run_case("event_function_xz", "function_xz=5 2 2\n");
}

#[test]
fn live_ref_formals_and_const_ref_function_reads_keep_dependencies() {
    run_case("event_formal_refs", "ref=11 function=1100\n");
}

#[test]
fn callback_helpers_with_locals_loops_and_nested_calls_evaluate_events() {
    super::sim_cli::run_case(
        "feature_completion/g1_06",
        "pure_callback_local_sum",
        "pure_callback_local_sum changes=5 qualifying=2\n",
        "",
        &[],
    );
}
