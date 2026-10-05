//! RTL-104: operator-overload leftovers (IEEE 1800-2009 11.11): values of
//! overloaded increments, targets evaluated once, expected types from a
//! relational operand, and the project ruling on package-import visibility.
//! Expected outputs are hand-derived; see the fixture readme.

use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_104";
const AMBIGUOUS_PLUS: &str =
    "ambiguous overload of operator '+'; use a cast to select the result type";
const INVALID_PLUS: &str = "invalid operands to binary expression ('p::s_t' and 'p::s_t')";

#[test]
fn increment_values_follow_prefix_and_postfix_rules() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_104/incdec_values.out");
    sim_cli::run_case(SUITE, "incdec_values", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "incdec_values", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "incdec_values", expected);
}

#[test]
fn update_targets_are_evaluated_once() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_104/single_evaluation.out");
    sim_cli::run_case(SUITE, "single_evaluation", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "single_evaluation", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "single_evaluation", expected);
}

#[test]
fn former_rtl_017_negatives_execute() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_104/rtl017_postfix_value.out");
    sim_cli::run_case(SUITE, "rtl017_postfix_value", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "rtl017_postfix_value", expected, &[], &[]);
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_104/rtl017_target_side_effects.out");
    sim_cli::run_case(SUITE, "rtl017_target_side_effects", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "rtl017_target_side_effects", expected, &[], &[]);
}

#[test]
fn relational_operands_supply_the_expected_type() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_104/relational_expected.out");
    sim_cli::run_case(SUITE, "relational_expected", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "relational_expected", expected, &[], &[]);
}

#[test]
fn package_overloads_follow_wildcard_imports() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_104/package_import.out");
    sim_cli::run_case(SUITE, "package_import", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "package_import", expected, &[], &[]);
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_104/package_import_unit.out");
    sim_cli::run_case(SUITE, "package_import_unit", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "package_import_unit", expected, &[], &[]);
}

#[test]
fn overloaded_updates_compose_with_processes() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_104/composition.out");
    sim_cli::run_case(SUITE, "composition", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "composition", expected, &[], &[]);
}

#[test]
fn neg_comparisons_without_one_unambiguous_operand_need_a_cast() {
    sim_cli::reject_case(SUITE, "neg_relational_both_ambiguous", AMBIGUOUS_PLUS);
    sim_cli::reject_case(SUITE, "neg_relational_nested", AMBIGUOUS_PLUS);
    sim_cli::reject_case(SUITE, "neg_relational_no_match", AMBIGUOUS_PLUS);
}

#[test]
fn neg_package_overloads_need_a_preceding_wildcard_import() {
    sim_cli::reject_case(SUITE, "neg_import_explicit", INVALID_PLUS);
    sim_cli::reject_case(SUITE, "neg_import_after_use", INVALID_PLUS);
    sim_cli::reject_case(
        SUITE,
        "neg_import_reexport",
        "invalid operands to binary expression ('p::s_t' and 'p::s_t')",
    );
    sim_cli::reject_case(SUITE, "neg_import_two_packages", AMBIGUOUS_PLUS);
}

#[test]
fn native_update_values_report_their_limit() {
    sim_cli::reject_case(
        SUITE,
        "limit_native_value",
        "an overloaded operator update whose value is used or whose target selector has side effects requires a target within the",
    );
}
