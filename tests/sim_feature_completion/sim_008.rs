//! SIM-008: native references and copy-out destinations: output and inout
//! actuals naming container elements. Oracles are derived by hand in the
//! fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_008";

#[test]
fn container_elements_receive_output_and_inout_copy_out() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/element_copy_out.out");
    sim_cli::run_case_backend_parity(SUITE, "element_copy_out", expected, &[], &[]);
    sim_cli::reject_case(
        SUITE,
        "neg_expression_element_output",
        "naming a queue, dynamic or associative array element is supported only when the call is a statement",
    );
}

#[test]
fn container_ref_formals_alias_the_caller_container() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/container_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "container_refs", expected, &[], &[]);
}

#[test]
fn element_refs_follow_elements_until_outdated() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/element_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "element_refs", expected, &[], &[]);
}

#[test]
fn native_record_ref_formals_alias_subroutine_records() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/record_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "record_refs", expected, &[], &[]);
    sim_cli::reject_case(
        SUITE,
        "neg_module_record_ref",
        "ref actual of native record formal `r` in `tb` must be a subroutine record variable of the same type (SIM-008)",
    );
}
