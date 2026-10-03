use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_003";

#[test]
fn adopted_selected_port() {
    sim_cli::run_case(
        SUITE,
        "ref_port_select",
        include_str!("../fixtures/sim/feature_completion/rtl_003/ref_port_select.out"),
        "",
        &[],
    );
}

#[test]
fn nested_ports_preserve_each_boundary() {
    sim_cli::run_case(
        SUITE,
        "nested_ports",
        include_str!("../fixtures/sim/feature_completion/rtl_003/nested_ports.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "nested_ports",
        include_str!("../fixtures/sim/feature_completion/rtl_003/nested_ports.out"),
    );
}

#[test]
fn selected_array_record_member() {
    sim_cli::run_case(
        SUITE,
        "array_record",
        include_str!("../fixtures/sim/feature_completion/rtl_003/array_record.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "array_record",
        include_str!("../fixtures/sim/feature_completion/rtl_003/array_record.out"),
    );
}

#[test]
fn bit_selected_port_bounds() {
    sim_cli::run_case(
        SUITE,
        "bit_port",
        include_str!("../fixtures/sim/feature_completion/rtl_003/bit_port.out"),
        "",
        &[],
    );
}

#[test]
fn selectors_capture_blocking_nba_mutation_and_copyout() {
    sim_cli::run_case(
        SUITE,
        "selector_capture",
        include_str!("../fixtures/sim/feature_completion/rtl_003/selector_capture.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "selector_capture",
        include_str!("../fixtures/sim/feature_completion/rtl_003/selector_capture.out"),
    );
}

#[test]
fn adopted_ref_scanner() {
    sim_cli::run_case(
        SUITE,
        "ref_scan",
        include_str!("../fixtures/sim/feature_completion/rtl_003/ref_scan.out"),
        "",
        &[],
    );
}

#[test]
fn illegal_temporary_ref() {
    sim_cli::reject_case(SUITE, "neg_ref_temporary", "ref");
}

#[test]
fn illegal_packed_subroutine_actual() {
    sim_cli::reject_case(SUITE, "neg_ref_packed_select", "ref");
}

#[test]
fn illegal_temporary_mutation() {
    sim_cli::reject_case(SUITE, "neg_mutation_temporary", "assignable");
}

#[test]
fn runtime_ref_rewiring_boundary() {
    sim_cli::reject_case(
        SUITE,
        "runtime_ref_port",
        "requires constant actual selectors",
    );
}

#[test]
fn illegal_packed_part_subroutine_actual() {
    sim_cli::reject_case(SUITE, "neg_ref_part", "ref");
}

#[test]
fn selected_port_limb_boundaries() {
    sim_cli::run_case(
        SUITE,
        "wide_port",
        include_str!("../fixtures/sim/feature_completion/rtl_003/wide_port.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "wide_port",
        include_str!("../fixtures/sim/feature_completion/rtl_003/wide_port.out"),
    );
}

#[test]
fn scanner_selected_reference_capture_and_failure() {
    sim_cli::run_case(
        SUITE,
        "ref_scan_selected",
        include_str!("../fixtures/sim/feature_completion/rtl_003/ref_scan_selected.out"),
        "",
        &[],
    );
    sim_cli::run_case_after_db_drop(
        SUITE,
        "ref_scan_selected",
        include_str!("../fixtures/sim/feature_completion/rtl_003/ref_scan_selected.out"),
    );
}
