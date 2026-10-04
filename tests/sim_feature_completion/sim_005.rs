//! SIM-005: real and shortreal references, real fixed arrays across calls,
//! numeric array methods and sampled values, and the real-operator
//! boundaries. Oracles are derived by hand in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_005";

#[test]
fn real_refs_alias_the_selected_numeric_cell() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_005/real_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "real_refs", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "real_refs", expected);
}

#[test]
fn real_element_output_and_inout_actuals_copy_back() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_005/real_element_copyout.out");
    sim_cli::run_case_backend_parity(SUITE, "real_element_copyout", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "real_element_copyout", expected);
}

#[test]
fn real_arrays_cross_subroutine_boundaries_by_value_and_reference() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_005/real_array_calls.out");
    sim_cli::run_case_backend_parity(SUITE, "real_array_calls", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "real_array_calls", expected);
}

#[test]
fn real_array_methods_compare_numbers() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_005/real_array_methods.out");
    sim_cli::run_case_backend_parity(SUITE, "real_array_methods", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "real_array_methods", expected);
}

#[test]
fn real_sampled_values_stay_numeric() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_005/real_sampled.out");
    sim_cli::run_case_backend_parity(SUITE, "real_sampled", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "real_sampled", expected);
}

#[test]
fn real_numeric_extremes_follow_the_conversion_policy() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_005/real_numeric_extremes.out");
    sim_cli::run_case_backend_parity(SUITE, "real_numeric_extremes", expected, &[], &[]);
    for level in ["O0", "O3"] {
        sim_cli::run_case_with_args(
            SUITE,
            "real_numeric_extremes",
            expected,
            "",
            &[],
            &["--model-opt-level", level],
        );
    }
}

#[test]
fn adopted_real_witnesses() {
    sim_cli::run_case(
        SUITE,
        "real_ref",
        include_str!("../fixtures/sim/feature_completion/sim_005/real_ref.out"),
        "",
        &[],
    );
    sim_cli::run_case(
        SUITE,
        "real_array_value",
        include_str!("../fixtures/sim/feature_completion/sim_005/real_array_value.out"),
        "",
        &[],
    );
    sim_cli::run_case(
        SUITE,
        "real_sample",
        include_str!("../fixtures/sim/feature_completion/sim_005/real_sample.out"),
        "",
        &[],
    );
}

#[test]
fn illegal_real_forms_stay_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_real_bitwise",
        "invalid operands to binary expression ('real' and 'int')",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_concat",
        "invalid operand type 'real' in concatenation",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_select",
        "value of type 'real' cannot be indexed",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_edge",
        "expression type 'real' is not integral",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_map",
        "array method 'sum' can only be called on unpacked arrays of integral values",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_nba",
        "nonblocking assignment to automatic variable 'x' is not allowed",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_ref_nba",
        "nonblocking assignment to automatic variable 'x' is not allowed",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_case_equality",
        "case equality on a real operand in `tb` is illegal",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_array_case_equality",
        "case equality on a real operand in `tb` is illegal",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_rose",
        "$rose of a real expression is illegal in `tb`",
    );
}

#[test]
fn real_container_element_refs_are_an_explicit_boundary() {
    sim_cli::reject_case(
        SUITE,
        "neg_real_queue_ref",
        "ref actual in `tb` names a real resizable-container element",
    );
}
