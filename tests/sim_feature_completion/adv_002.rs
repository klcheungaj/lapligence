//! ADV-002: distributed continuous-assignment, gate, UDP and net delays.
//! Oracles are hand-derived from IEEE 1364-2001 4.3, 6.1.3 and 7.14 (Table
//! 39) and IEEE 1800-2009 10.3.3, 11.12 and 28.16 (Table 28-9), as quoted in
//! the fixture readme. Verilog sources run in both editions; every positive
//! case runs both optimizer modes on every value backend.
use super::sim_cli;

const SUITE: &str = "feature_completion/adv_002";
const BOTH: &[&str] = &["v2001", "sv2009"];

#[test]
fn min_typ_max_expressions_select_the_typical_value() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_002/mintypmax_select.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "mintypmax_select.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn vector_targets_take_one_delay_for_the_whole_value() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/adv_002/vector_transitions.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "vector_transitions.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn vector_net_delays_schedule_the_whole_vector() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_002/net_delay_bits.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "net_delay_bits.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "net_delay_bits.v", expected);
}

#[test]
fn declaration_assignment_delays_belong_to_their_driver() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_002/decl_assign_delay.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "decl_assign_delay.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn inertial_pulses_below_the_delay_are_suppressed() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_002/pulse_widths.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "pulse_widths.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn gate_delays_follow_the_transition_table() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_002/gate_forms.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "gate_forms.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "gate_forms.v", expected);
}

#[test]
fn net_array_declaration_delays_apply_to_every_element() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_002/net_array_delays.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "net_array_delays.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "net_array_delays.v", expected);
    let ports = include_str!("../fixtures/sim/feature_completion/adv_002/net_array_ports.out");
    sim_cli::run_case_backend_parity(SUITE, "net_array_ports", ports, &[], &[]);
}

#[test]
fn concatenation_targets_are_one_vector_driver() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_002/concat_targets.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "concat_targets.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "concat_targets.v", expected);
}

#[test]
fn pattern_members_are_separate_drivers_and_alias_splits_delay_once() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/adv_002/pattern_alias_targets.out");
    sim_cli::run_case_backend_parity(SUITE, "pattern_alias_targets", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "pattern_alias_targets", expected);
}

#[test]
fn neg_delayed_descriptor_pattern_member() {
    sim_cli::reject_case(
        SUITE,
        "neg_delayed_descriptor_pattern",
        "delayed continuous assignment-pattern leaf in `tb` targets a descriptor-backed array row",
    );
}

#[test]
fn neg_negative_net_delay() {
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_net_delay_negative.v",
            "net propagation delay for `bad`",
            &["--edition", edition],
        );
    }
}

#[test]
fn runtime_driver_delays_are_evaluated_when_scheduled() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/adv_002/runtime_driver_delays.out");
    sim_cli::run_case_backend_parity(SUITE, "runtime_driver_delays", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "runtime_driver_delays", expected);
}

#[test]
fn runtime_net_delays_apply_the_current_value() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/adv_002/runtime_net_delays.out");
    sim_cli::run_case_backend_parity(SUITE, "runtime_net_delays", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "runtime_net_delays", expected);
}

#[test]
fn neg_runtime_delays_in_verilog_2001() {
    const CONSTANT: &str = "IEEE 1364-2001 7.14.1 requires constant delay expressions";
    const V2001: &[&str] = &["--edition", "v2001"];
    sim_cli::reject_case_with_args(SUITE, "neg_runtime_delays_v2001.v", CONSTANT, V2001);
    sim_cli::reject_case_with_args(SUITE, "neg_runtime_gate_delay_v2001.v", CONSTANT, V2001);
    sim_cli::reject_case_with_args(SUITE, "neg_runtime_net_delay_v2001.v", CONSTANT, V2001);
}

#[test]
fn neg_runtime_net_delay_with_another_declared_delay() {
    sim_cli::reject_case(
        SUITE,
        "neg_runtime_net_delay_conflict",
        "combines a runtime-valued delay with another declared net delay",
    );
}
