//! RTL-105: frontend admission of `inout uwire` formals, concatenated uwire
//! inout actuals, and continuous assignments to variables through runtime
//! selects. Oracles are derived by hand from IEEE 1800-2009 (fixture readme).

use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_105";

#[test]
fn inout_uwire_formals_collapse_with_one_driver() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_105/uwire_inout_formal.out");
    sim_cli::run_case(SUITE, "uwire_inout_formal", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "uwire_inout_formal", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "uwire_inout_formal", expected);
}

#[test]
fn concatenated_uwire_inout_actuals_are_not_drivers() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_105/uwire_concat_inout.out");
    sim_cli::run_case(SUITE, "uwire_concat_inout", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "uwire_concat_inout", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "uwire_concat_inout", expected);
}

#[test]
fn neg_uwire_formals_and_concatenations_keep_one_driver() {
    const COLLAPSED: &str = "a collapsed uwire net has 2 drivers";
    sim_cli::reject_case(SUITE, "neg_uwire_formal_two_drivers", COLLAPSED);
    sim_cli::reject_case(SUITE, "neg_uwire_concat_drivers", COLLAPSED);
    sim_cli::reject_case(
        SUITE,
        "neg_uwire_formal_local_drivers",
        "'uwire' net 'p' cannot have multiple drivers",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_uwire_formal_pass_switch",
        "'uwire' net 'p' cannot be connected to 'inout' port",
    );
}

#[test]
fn continuous_assignment_to_variable_admits_runtime_selects() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/rtl_105/assign_runtime_select.out");
    sim_cli::run_case(SUITE, "assign_runtime_select", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "assign_runtime_select", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "assign_runtime_select", expected);
}

#[test]
fn runtime_selected_continuous_assignments_compose() {
    let expected = include_str!(
        "../fixtures/sim/feature_completion/rtl_105/assign_runtime_select_composition.out"
    );
    sim_cli::run_case(
        SUITE,
        "assign_runtime_select_composition",
        expected,
        "",
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "assign_runtime_select_composition",
        expected,
        &[],
        &[],
    );
    sim_cli::run_case_after_db_drop(SUITE, "assign_runtime_select_composition", expected);
}

/// The LRM leaves a pending delayed update whose selector changes first
/// undefined; this pins the chosen behaviour (fixture readme).
#[test]
fn runtime_selected_delayed_update_follows_newest_evaluation() {
    let expected = include_str!(
        "../fixtures/sim/feature_completion/rtl_105/assign_runtime_select_retarget.out"
    );
    sim_cli::run_case(SUITE, "assign_runtime_select_retarget", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "assign_runtime_select_retarget", expected, &[], &[]);
}

#[test]
fn neg_runtime_selected_continuous_assignments() {
    const CONSTANT: &str =
        "reference to non-constant variable 'i' is not allowed in a constant expression";
    sim_cli::reject_case(SUITE, "neg_assign_runtime_select_net", CONSTANT);
    sim_cli::reject_case(SUITE, "neg_assign_runtime_select_gate", CONSTANT);
    // The longest static prefix of a runtime select is the written storage.
    sim_cli::reject_case(
        SUITE,
        "neg_assign_runtime_select_procedural",
        "variable storage `tb.a` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_assign_runtime_select_row",
        "variable storage `tb.rows[1][0] through tb.rows[1][2]` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_assign_runtime_select_two",
        "multiple continuous assignments to variable storage `tb.a`",
    );
}
