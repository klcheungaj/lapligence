//! SIM-021: runtime-native operator overloading (IEEE 1800-2009 11.11):
//! records with string, container and handle members, unions, real operands,
//! result types chosen by context, update values on native targets, and the
//! declarations and bound functions that are rejected. Expected outputs are
//! hand-derived; see the fixture readme. Positives run in both optimizer
//! modes on the legacy, compact/portable and compact/GMP value backends.

use super::sim_cli;

const SUITE: &str = "feature_completion/sim_021";
const BUILTIN_LEGAL: &str =
    "is already legal for the types of this overload prototype and cannot be overloaded";
const AMBIGUOUS_PLUS: &str =
    "ambiguous overload of operator '+'; use a cast to select the result type";
const PROTOTYPE_MISMATCH: &str = "function 'fa' does not match its operator overload prototype";
const TIMED: &str = "statements that pass time are not allowed in this context";

#[test]
fn native_record_results_are_independently_owned() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_021/native_pipeline.out");
    sim_cli::run_case_backend_parity(SUITE, "native_pipeline", expected, &[], &[]);
}

#[test]
fn expected_result_types_select_native_record_overloads() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_021/native_contexts.out");
    sim_cli::run_case_backend_parity(SUITE, "native_contexts", expected, &[], &[]);
}

#[test]
fn unions_reals_and_mixed_operands_overload() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_021/operand_kinds.out");
    sim_cli::run_case_backend_parity(SUITE, "operand_kinds", expected, &[], &[]);
}

#[test]
fn native_and_oversized_update_values() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_021/native_update_values.out");
    sim_cli::run_case_backend_parity(SUITE, "native_update_values", expected, &[], &[]);
}

#[test]
fn casts_select_nested_native_overloads() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_021/nested_casts.out");
    sim_cli::run_case_backend_parity(SUITE, "nested_casts", expected, &[], &[]);
}

#[test]
fn bound_functions_keep_legal_function_effects() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_021/legal_effects.out");
    sim_cli::run_case_backend_parity(SUITE, "legal_effects", expected, &[], &[]);
}

#[test]
fn neg_already_legal_operators_cannot_be_overloaded() {
    let legal = |operator: &str| format!("operator {operator} {BUILTIN_LEGAL}");
    sim_cli::reject_case(SUITE, "neg_builtin_add", &legal("'+'"));
    sim_cli::reject_case(SUITE, "neg_builtin_copy", &legal("'='"));
    sim_cli::reject_case(SUITE, "neg_builtin_equality", &legal("'!='"));
    sim_cli::reject_case(SUITE, "neg_builtin_conversion", &legal("'='"));
    sim_cli::reject_case(SUITE, "neg_builtin_string_compare", &legal("'<'"));
}

#[test]
fn neg_bound_functions_cannot_pass_time() {
    sim_cli::reject_case(SUITE, "neg_timed_function", TIMED);
    sim_cli::reject_case(SUITE, "neg_event_wait_function", TIMED);
    sim_cli::reject_case(SUITE, "neg_task_binding", PROTOTYPE_MISMATCH);
    sim_cli::reject_case(SUITE, "neg_output_formal", PROTOTYPE_MISMATCH);
}

#[test]
fn neg_native_overloads_need_a_visible_unambiguous_prototype() {
    sim_cli::reject_case(
        SUITE,
        "neg_invisible_declaration",
        "invalid operands to binary expression ('t_t' and 't_t')",
    );
    sim_cli::reject_case(SUITE, "neg_nested_ambiguous_native", AMBIGUOUS_PLUS);
    sim_cli::reject_case(SUITE, "neg_self_determined_native", AMBIGUOUS_PLUS);
}

#[test]
fn neg_callback_contexts_keep_their_effect_restrictions() {
    sim_cli::reject_case(
        SUITE,
        "neg_monitor_effect",
        "`$monitor`/`$strobe` arguments are evaluated in the read-only Postponed region",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_assertion_effect",
        "unsupported sampled consequent expression in concurrent assertion",
    );
}

#[test]
fn native_update_value_arguments_report_their_limit() {
    sim_cli::reject_case(
        SUITE,
        "limit_native_value_argument",
        "an overloaded operator update on a target above the 1048575-bit packed value limit or with native members yields a value only as the right-hand side of an assignment",
    );
}
