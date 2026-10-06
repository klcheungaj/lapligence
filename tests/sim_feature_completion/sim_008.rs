//! SIM-008: native references and copy-out destinations: output and inout
//! actuals naming container elements, and native record `ref` formals.
//! Oracles are derived by hand in the fixture readme.
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
}

#[test]
fn native_record_ref_formals_bind_module_and_block_records() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_008/module_record_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "module_record_refs", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "module_record_refs", expected);
}

#[test]
fn timed_record_refs_share_the_record_with_other_processes() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/timed_record_refs.out");
    sim_cli::run_case_backend_parity(SUITE, "timed_record_refs", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "timed_record_refs", expected);
}

#[test]
fn unbindable_record_ref_actuals_are_rejected_explicitly() {
    const UNBOUND: &str = "ref actual of native record formal `r` in `tb` must be a whole subroutine record, or a module, static or procedural-block record or a constant member/index selection of one, of the same type (SIM-008)";
    sim_cli::reject_case(SUITE, "neg_record_ref_element", UNBOUND);
    sim_cli::reject_case(SUITE, "neg_record_ref_member", UNBOUND);
    sim_cli::reject_case(
        SUITE,
        "neg_record_ref_method",
        "a module, static or procedural-block record as the actual of native record `ref` formal `r` of method `f` in `tb` is not supported (SIM-008)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_record_ref_const_write",
        "cannot assign to read-only variable 'r'",
    );
}

#[test]
fn input_system_functions_store_into_container_elements() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/scan_elements.out");
    sim_cli::run_case_backend_parity(SUITE, "scan_elements", expected, &[], &[]);
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/plusarg_elements.out");
    sim_cli::run_case_backend_parity(
        SUITE,
        "plusarg_elements",
        expected,
        &["--append-plusarg", "+N=4", "--append-plusarg", "+H=1f"],
        &[],
    );
    sim_cli::reject_case(
        SUITE,
        "neg_string_element_scan",
        "string element of a queue, dynamic or associative array as a ref actual or input destination in `tb` is not supported (SIM-008)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_real_element_scan",
        "real element of a queue, dynamic or associative array as an input destination in `tb` is not supported (SIM-008)",
    );
}

#[test]
fn ref_formals_mix_with_inputs_and_recursive_forwarding() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_008/mixed_ref_formals.out");
    sim_cli::run_case_backend_parity(SUITE, "mixed_ref_formals", expected, &[], &[]);
}
