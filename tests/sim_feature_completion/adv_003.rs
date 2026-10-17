//! ADV-003: sequential UDP tables and initialization. Oracles are
//! hand-derived from IEEE 1364-2001 8.1-8.8 and IEEE 1800-2009 29.3-29.10, as
//! quoted in the fixture readme. Every source is Verilog and runs in both
//! editions; every positive case runs both optimizer modes on every value
//! backend.
use super::sim_cli;

const SUITE: &str = "feature_completion/adv_003";
const BOTH: &[&str] = &["v2001", "sv2009"];

#[test]
fn level_sensitive_latch_holds_and_defaults_to_x() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/latch.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(SUITE, "latch.v", expected, &["--edition", edition], &[]);
    }
}

#[test]
fn every_edge_symbol_family_over_all_input_transitions() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/edge_symbols.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "edge_symbols.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "edge_symbols.v", expected);
}

#[test]
fn flip_flop_with_explicit_transitions() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/flip_flop.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "flip_flop.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn mixed_tables_let_level_rows_dominate() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/mixed_tables.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "mixed_tables.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn initial_values_are_not_delayed() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/initial_delay.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "initial_delay.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
    sim_cli::run_case_after_db_drop(SUITE, "initial_delay.v", expected);
}

#[test]
fn instance_arrays_keep_independent_state() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/instance_array.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "instance_array.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn repeated_same_slot_changes_are_separate_transitions() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/same_slot.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "same_slot.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn outputs_drive_with_strengths_and_delays() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/strengths.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "strengths.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn time_zero_inputs_start_unknown() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/time_zero.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "time_zero.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn wide_tables_scan_their_rows() {
    let expected = include_str!("../fixtures/sim/feature_completion/adv_003/wide_table.out");
    for edition in BOTH {
        sim_cli::run_case_backend_parity(
            SUITE,
            "wide_table.v",
            expected,
            &["--edition", edition],
            &[],
        );
    }
}

#[test]
fn neg_combinational_tables_reject_sequential_fields() {
    for edition in BOTH {
        sim_cli::reject_case_with_args(SUITE, "neg_edge_in_combinational.v", "neg_edge_in_combinational.v:7:9 edge transition can only be used in a sequential primitive", &["--edition", edition]);
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(SUITE, "neg_shorthand_in_combinational.v", "neg_shorthand_in_combinational.v:7:9 edge transition can only be used in a sequential primitive", &["--edition", edition]);
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(SUITE, "neg_initial_in_combinational.v", "neg_initial_in_combinational.v:6:5 combinational primitives cannot contain an initial statement", &["--edition", edition]);
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(SUITE, "neg_state_in_combinational.v", "neg_state_in_combinational.v:7:13 combinational primitive row should not have a current state field", &["--edition", edition]);
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(SUITE, "neg_no_change_in_combinational.v", "neg_no_change_in_combinational.v:7:13 '-' is only allowed in the output field of a sequential primitive", &["--edition", edition]);
    }
}

#[test]
fn neg_sequential_rows_need_state_and_one_transition() {
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_missing_state.v",
            "neg_missing_state.v:7:15 sequential primitive row is missing a current state field",
            &["--edition", edition],
        );
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_two_transitions.v",
            "neg_two_transitions.v:7:14 cannot have more than one input transition per row",
            &["--edition", edition],
        );
    }
}

#[test]
fn neg_contradictory_entries() {
    const DUPLICATE: &str =
        "primitive table row duplicates a set of inputs with a different specified output value";
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_conflicting_edges.v",
            &format!("neg_conflicting_edges.v:8:9 {DUPLICATE}"),
            &["--edition", edition],
        );
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_conflicting_no_change.v",
            &format!("neg_conflicting_no_change.v:8:9 {DUPLICATE}"),
            &["--edition", edition],
        );
    }
}

#[test]
fn neg_port_and_table_arity() {
    for edition in BOTH {
        sim_cli::reject_case_with_args(SUITE, "neg_row_arity.v", "neg_row_arity.v:7:9 incorrect number of input fields in table row; have 1 but expect 2", &["--edition", edition]);
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(SUITE, "neg_instance_arity.v", "neg_instance_arity.v:14:18 wrong number of port connections for 'two_inputs' instance (2 given, expected 3)", &["--edition", edition]);
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_vector_terminal.v",
            "input terminal 2 of sequential UDP `u` in `tb` must be scalar",
            &["--edition", edition],
        );
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_vector_terminal.v",
            "neg_vector_terminal.v:16:",
            &["--edition", edition],
        );
    }
}

#[test]
fn neg_output_port_and_initial_rules() {
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_output_not_first.v",
            "neg_output_not_first.v:4:11 the first port in a primitive must be an output port",
            &["--edition", edition],
        );
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_reg_input.v",
            "neg_reg_input.v:5:21 input port 'b' cannot be declared as a 'reg'",
            &["--edition", edition],
        );
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(SUITE, "neg_initial_value.v", "neg_initial_value.v:6:17 initial value for a primitive output must be a single-bit binary literal", &["--edition", edition]);
    }
    for edition in BOTH {
        sim_cli::reject_case_with_args(
            SUITE,
            "neg_initial_target.v",
            "neg_initial_target.v:6:13 'a' is not this primitive's output port",
            &["--edition", edition],
        );
    }
}
