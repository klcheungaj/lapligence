use super::sim_cli;

const SUITE: &str = "feature_completion/rtl_102";

#[test]
fn audit_probe_p02_reads_and_writes_through_port_expressions() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_102/probe_p02.out");
    sim_cli::run_case(SUITE, "probe_p02", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "probe_p02", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "probe_p02", expected);
}

#[test]
fn continuous_views_cross_ports_arrays_generate_and_forwarding() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_102/continuous_views.out");
    sim_cli::run_case(SUITE, "continuous_views", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "continuous_views", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "continuous_views", expected);
}

#[test]
fn procedural_views_select_compose_and_wake_readers() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_102/procedural_views.out");
    sim_cli::run_case(SUITE, "procedural_views", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "procedural_views", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "procedural_views", expected);
}

#[test]
fn inout_views_resolve_with_other_interface_net_drivers() {
    let expected = include_str!("../fixtures/sim/feature_completion/rtl_102/inout_views.out");
    sim_cli::run_case(SUITE, "inout_views", expected, "", &[]);
    sim_cli::run_case_backend_parity(SUITE, "inout_views", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "inout_views", expected);
}

#[test]
fn neg_illegal_port_expression_forms() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_not_lvalue",
        "expression is not assignable",
    );
    sim_cli::reject_case(SUITE, "neg_input_write", "cannot assign to input port 'p'");
    sim_cli::reject_case(
        SUITE,
        "neg_runtime_selector",
        "reference to non-constant variable 'idx' is not allowed in a constant expression",
    );
}

#[test]
fn neg_writer_conflicts_through_port_expressions() {
    sim_cli::reject_case(
        SUITE,
        "neg_continuous_and_procedural",
        "variable storage `tb.bi.a[0 +: 4]` has both a continuous assignment",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_two_continuous",
        "multiple continuous assignments to variable storage `tb.bi.a[0 +: 4]`",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_concat_select_conflict",
        "variable storage `tb.bi.a[0]` has both a continuous assignment",
    );
}

#[test]
fn neg_virtual_interface_expression_port_is_deferred() {
    sim_cli::reject_case(
        SUITE,
        "neg_virtual_interface_expression_port",
        "modport expression port `p` is not supported through virtual interface view",
    );
}
