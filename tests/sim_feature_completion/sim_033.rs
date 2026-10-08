//! SIM-033: clocking input expressions and legal sample types. Oracles are
//! derived by hand in the fixture readme from IEEE 1800-2009 §§3.14.3,
//! 14.3-14.15.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_033";

/// Execute after the frontend snapshot and owned Db are destroyed, so clocking
/// expressions, sample types and source sites come from owned data only.
fn after_db_drop(fixture: &str, expected: &str, expected_stderr: &str) {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        fixture,
        llg::core::compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        expected,
        expected_stderr,
    );
}

#[test]
fn input_skews_sample_preponed_observed_and_history_values() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/skew_timeline.out");
    sim_cli::run_case_backend_parity(SUITE, "skew_timeline", expected, &[], &[]);
    after_db_drop(
        "skew_timeline",
        expected,
        "llg: $finish at time 41 at tb:38:8\n",
    );
}

#[test]
fn select_concatenation_computed_and_real_inputs_sample_their_expression() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/expressions.out");
    sim_cli::run_case_backend_parity(SUITE, "expressions", expected, &[], &[]);
    after_db_drop(
        "expressions",
        expected,
        "llg: $finish at time 10 at tb:64:5\n",
    );
}

#[test]
fn interface_clocking_inputs_read_through_ports_views_and_virtual_handles() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/interfaces.out");
    sim_cli::run_case_backend_parity(SUITE, "interfaces", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "interfaces", expected);
}

#[test]
fn default_clocking_references_cycles_and_global_clocking() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/defaults.out");
    sim_cli::run_case_backend_parity(SUITE, "defaults", expected, &[], &[]);
}

// FND-002 witnesses, source unchanged.

#[test]
fn const_skew_witness() {
    // L-F12-04-04: a parameter or literal skew is the legal counterpart of a
    // runtime skew.
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_033/const_skew_witness.out");
    sim_cli::run_case_backend_parity(SUITE, "const_skew_witness", expected, &[], &[]);
}

#[test]
fn expression_witness() {
    // L-F12-04-02: a computed clocking input expression.
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_033/expression_witness.out");
    sim_cli::run_case_backend_parity(SUITE, "expression_witness", expected, &[], &[]);
}

#[test]
fn real_witness() {
    // L-F12-04-01: a real clocking input.
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/real_witness.out");
    sim_cli::run_case_backend_parity(SUITE, "real_witness", expected, &[], &[]);
}

#[test]
fn cross_clock_cycle_witness() {
    // L-F06-07-02, L-F12-04-05: a cycle delay on the default clock followed
    // by a drive of another block's output.
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_033/cross_clock_cycle_witness.out");
    sim_cli::run_case_backend_parity(SUITE, "cross_clock_cycle_witness", expected, &[], &[]);
}

#[test]
fn neg_dynamic_skew() {
    // SV 14.4: a skew is a constant expression; FND-002 neg_dynamic_skew.
    sim_cli::reject_case(
        SUITE,
        "neg_dynamic_skew",
        "reference to non-constant variable 'n' is not allowed in a constant expression",
    );
}

#[test]
fn neg_cycle_no_default() {
    // SV 14.11; FND-002 neg_cycle_no_default.
    sim_cli::reject_case(
        SUITE,
        "neg_cycle_no_default",
        "cycle delay cannot be used because no default clocking has been specified",
    );
}

#[test]
fn neg_compound_drive() {
    // SV 14.16; FND-002 neg_clocking_compound.
    sim_cli::reject_case(
        SUITE,
        "neg_compound_drive",
        "can only be written via a synchronous drive",
    );
}

#[test]
fn neg_concat_drive() {
    // SV 14.16; FND-002 neg_clocking_concat.
    sim_cli::reject_case(
        SUITE,
        "neg_concat_drive",
        "cannot be part of a concatenation or assignment pattern lvalue",
    );
}

#[test]
fn neg_input_write() {
    sim_cli::reject_case(
        SUITE,
        "neg_input_write",
        "cannot write to input clocking signal 'x'",
    );
}

#[test]
fn neg_output_read() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_read",
        "cannot read from output clocking signal 'x'",
    );
}

#[test]
fn neg_output_expression() {
    sim_cli::reject_case(
        SUITE,
        "neg_output_expression",
        "expression is not assignable",
    );
}

#[test]
fn neg_automatic_source() {
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_source",
        "cannot refer to automatic variable via hierarchical reference",
    );
}

#[test]
fn neg_unresolved_member() {
    sim_cli::reject_case(
        SUITE,
        "neg_unresolved_member",
        "could not resolve hierarchical path name 'y'",
    );
}

#[test]
fn neg_chandle_input() {
    sim_cli::reject_case(
        SUITE,
        "neg_chandle_input",
        "chandle clocking input `c` in `tb` is illegal",
    );
}

// Legal clocking inputs whose sampled storage is not implemented are
// rejected, never sampled as packed values.

#[test]
fn neg_unpacked_input() {
    sim_cli::reject_case(
        SUITE,
        "neg_unpacked_input",
        "clocking input `arr` in `tb` has type `int$[0:1]`; sampled clockvars are limited to \
         packed integral, real and shortreal values",
    );
}

#[test]
fn neg_string_input() {
    sim_cli::reject_case(
        SUITE,
        "neg_string_input",
        "clocking input `s` in `tb` has type `string`; sampled clockvars are limited to \
         packed integral, real and shortreal values",
    );
}

#[test]
fn neg_input_edge_skew() {
    sim_cli::reject_case(
        SUITE,
        "neg_input_edge_skew",
        "edge-qualified input skew of clocking input `x` in `tb` is not supported",
    );
}

#[test]
fn neg_vif_clocking_event() {
    sim_cli::reject_case(
        SUITE,
        "neg_vif_clocking_event",
        "clocking block event `cb` is not supported through virtual interface `bus.tb_mp` in `tb`",
    );
}
