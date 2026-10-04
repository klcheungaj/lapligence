use super::sim_cli;

const SUITE: &str = "feature_completion/ki_packed_range";

#[test]
fn procedural_reads_and_writes_select_whole_elements() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_packed_range/procedural_selects.out");
    sim_cli::run_case(SUITE, "procedural_selects", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "procedural_selects", expected, &[], &[]);
}

#[test]
fn continuous_port_alias_force_and_sensitivity_views() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_packed_range/continuous_ports.out");
    sim_cli::run_case(SUITE, "continuous_ports", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "continuous_ports", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "continuous_ports", expected);
}

#[test]
fn modport_expression_port_over_an_element_range() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_packed_range/modport_ranges.out");
    sim_cli::run_case(SUITE, "modport_ranges", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "modport_ranges", expected, &[], &[]);
}

#[test]
fn formals_locals_and_ref_ports_select_whole_elements() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/ki_packed_range/subroutine_views.out");
    sim_cli::run_case(SUITE, "subroutine_views", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "subroutine_views", expected, &[], &[]);
}

#[test]
fn neg_reversed_and_runtime_width_ranges() {
    sim_cli::reject_case(
        SUITE,
        "neg_reversed_range",
        "range of selection [2:3] from 'logic[3:0][7:0]' is reversed",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_runtime_width",
        "reference to non-constant variable 'n' is not allowed in a constant expression",
    );
}
