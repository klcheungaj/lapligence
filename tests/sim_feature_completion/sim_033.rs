//! SIM-033: clocking input expressions and legal sample types. Oracles are
//! derived by hand in the fixture readme from IEEE 1800-2009 §§3.14.3,
//! 14.3-14.15.
use super::{sim_cli, sim_harness};

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
        "INFO: skew_timeline.sv:38:8: [SIM-001] (tb @ 41 ns) $finish called\n",
    );
}

#[test]
fn select_concatenation_computed_and_real_inputs_sample_their_expression() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/expressions.out");
    sim_cli::run_case_backend_parity(SUITE, "expressions", expected, &[], &[]);
    after_db_drop(
        "expressions",
        expected,
        "INFO: expressions.sv:64:5: [SIM-001] (tb @ 10 ns) $finish called\n",
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

// K1: unpacked fixed-size clockvars and edge input skews. Expected outputs
// are industry-standard behaviour's (readme), checked against the clauses.

#[test]
fn unpacked_inputs_sample_whole_values() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/unpacked_inputs.out");
    sim_cli::run_case_backend_parity(SUITE, "unpacked_inputs", expected, &[], &[]);
}

#[test]
fn unpacked_union_input_samples_whole_value() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_033/unpacked_union_input.out");
    sim_cli::run_case_backend_parity(SUITE, "unpacked_union_input", expected, &[], &[]);
}

#[test]
fn record_inputs_sample_the_same_in_both_storage_forms() {
    // 16 cells make the member array one column, 3 cells one leaf per cell
    // (LLG_RECORD_COLUMN_MIN_CELLS); both print the same.
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/record_inputs.out");
    for cells in ["N=16", "N=3"] {
        sim_cli::run_case_backend_parity(SUITE, "record_inputs", expected, &["-D", cells], &[]);
    }
}

#[test]
fn unpacked_inputs_read_through_virtual_interface() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_033/vif_unpacked_inputs.out");
    sim_cli::run_case_backend_parity(SUITE, "vif_unpacked_inputs", expected, &[], &[]);
}

#[test]
fn edge_skews_sample_at_the_clock_edge() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/edge_skews.out");
    sim_cli::run_case_backend_parity(SUITE, "edge_skews", expected, &[], &[]);
}

#[test]
fn edge_skew_forms() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_033/edge_skew_forms.out");
    sim_cli::run_case_backend_parity(SUITE, "edge_skew_forms", expected, &[], &[]);
}

const UNSERVED_TYPES: &str = "string, class handle, event, queue, dynamic array and \
                              associative array clockvars are not supported";

#[test]
fn neg_string_input() {
    // Decision K1-D1.
    sim_cli::reject_case(
        SUITE,
        "neg_string_input",
        &format!("neg_string_input.sv:7:11 clocking input `s` in `tb` has type `string`; {UNSERVED_TYPES}"),
    );
}

#[test]
fn neg_class_input() {
    sim_cli::reject_case(
        SUITE,
        "neg_class_input",
        &format!(
            "neg_class_input.sv:9:11 clocking input `h` in `tb` has type `C`; {UNSERVED_TYPES}"
        ),
    );
}

#[test]
fn neg_event_input() {
    sim_cli::reject_case(
        SUITE,
        "neg_event_input",
        &format!("neg_event_input.sv:6:11 clocking input `ev` in `tb` has type `event`; {UNSERVED_TYPES}"),
    );
}

#[test]
fn neg_queue_input() {
    sim_cli::reject_case(
        SUITE,
        "neg_queue_input",
        &format!("neg_queue_input.sv:7:11 clocking input `q` in `tb` has type `int$[$]`; {UNSERVED_TYPES}"),
    );
}

#[test]
fn neg_real_array_input() {
    sim_cli::reject_case(
        SUITE,
        "neg_real_array_input",
        "neg_real_array_input.sv:7:11 clocking input `r` in `tb` has type `real$[0:1]`; an \
         unpacked clockvar with a real, string, class handle, event or dynamic array element or \
         member is not supported",
    );
}

#[test]
fn neg_input_both_edges_skew() {
    sim_cli::reject_case(
        SUITE,
        "neg_input_both_edges_skew",
        "neg_input_both_edges_skew.sv:7:16 an `edge` input skew of clocking input `x` in `tb` is \
         not supported",
    );
}

#[test]
fn neg_edge_skew_delay() {
    sim_cli::reject_case(
        SUITE,
        "neg_edge_skew_delay",
        "neg_edge_skew_delay.sv:6:22 an edge input skew with a delay of clocking input `x` in \
         `tb` is not supported",
    );
}

#[test]
fn neg_edge_skew_event() {
    sim_cli::reject_case(
        SUITE,
        "neg_edge_skew_event",
        "neg_edge_skew_event.sv:7:19 an edge input skew of clocking input `x` in `tb` needs a \
         clocking event on one signal",
    );
}

#[test]
fn vif_clocking_event() {
    // The former neg_vif_clocking_event: `@(vp.cb)` through a view.
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_033/vif_clocking_event.out");
    sim_cli::run_case_backend_parity(SUITE, "vif_clocking_event", expected, &[], &[]);
}

#[test]
fn vif_clocking_events_follow_the_bound_instance() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_033/vif_clocking_events.out");
    sim_cli::run_case_backend_parity(SUITE, "vif_clocking_events", expected, &[], &[]);
}

#[test]
fn vif_clocking_event_through_null_handle_stops() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_033/vif_clocking_event_null.out");
    sim_cli::run_case_checked_matrix(SUITE, "vif_clocking_event_null", &[], &|label, output| {
        assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{label}");
        assert!(
            sim_harness::mentions(
                &String::from_utf8_lossy(&output.stderr),
                "virtual interface access failed: virtual interface clocking block event at \
                 tb:18:7"
            ),
            "{label}: {output:?}"
        );
    });
}
