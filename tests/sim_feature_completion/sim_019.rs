//! SIM-019: container and native-array methods, foreach and queries.

use super::sim_cli;

const SUITE: &str = "feature_completion/sim_019";

#[test]
fn locators_over_native_elements_and_sizes() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_019/locators_elements.out");
    sim_cli::run_case(SUITE, "locators_elements", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "locators_elements", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "locators_elements", expected);
}

#[test]
fn index_results_keep_key_types_and_declared_indices() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_019/index_keys.out");
    sim_cli::run_case(SUITE, "index_keys", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "index_keys", expected, &[], &[]);
}

#[test]
fn with_expressions_run_once_per_element_and_capture_state() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_019/callbacks.out");
    sim_cli::run_case(SUITE, "callbacks", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "callbacks", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "callbacks", expected);
}

#[test]
fn ordering_pops_copies_and_nested_queries() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_019/orders_copies.out");
    sim_cli::run_case(SUITE, "orders_copies", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "orders_copies", expected, &[], &[]);
}

#[test]
fn reductions_take_width_from_with_expressions() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_019/reductions.out");
    sim_cli::run_case(SUITE, "reductions", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "reductions", expected, &[], &[]);
}

#[test]
fn foreach_over_dynamic_and_nested_receivers() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_019/foreach_dynamic.out");
    sim_cli::run_case(SUITE, "foreach_dynamic", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "foreach_dynamic", expected, &[], &[]);
}

#[test]
fn large_receivers_use_linear_scans_and_n_log_n_orders() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_019/large.out");
    sim_cli::run_case(SUITE, "large", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "large", expected, &[], &[]);
}

#[test]
fn neg_illegal_methods_reductions_and_arguments() {
    sim_cli::reject_case(
        SUITE,
        "neg_wildcard_index",
        "'find_index' cannot be called with an associative array with wildcard index type",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_wildcard_foreach",
        "foreach loops cannot be used with associative arrays that have a wildcard index",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_reduction",
        "array method 'sum' can only be called on unpacked arrays of integral values",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_string_reduction",
        "array method 'sum' can only be called on unpacked arrays of integral values",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_find_without_with",
        "'find' requires 'with' expression for searching",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_record_max",
        "array method 'max' can only be called on unpacked arrays of comparable values",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_string_key_result",
        "no implicit conversion from 'string$[$]' to 'int$[$]'",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_sort_argument",
        "cannot provide arguments to 'sort' without corresponding 'with' clause",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_shuffle_with",
        "cannot use 'with' expression with 'shuffle'",
    );
}
