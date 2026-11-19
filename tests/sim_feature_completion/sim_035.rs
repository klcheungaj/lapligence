//! SIM-035: ordinary sampled-value functions and complex clocks. Oracles are
//! derived by hand in the fixture readme from IEEE 1800-2009 §§16.5-16.6,
//! 16.9.3, 9.4.2 and 14.12-14.14.
use super::sim_cli;

const SUITE: &str = "feature_completion/sim_035";

#[test]
fn procedural_histories_follow_initial_values_depth_and_gates() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_035/procedural_history.out");
    sim_cli::run_case_backend_parity(SUITE, "procedural_history", expected, &[], &[]);
}

#[test]
fn complex_clocking_events_sample_on_every_tick() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_035/complex_clocks.out");
    sim_cli::run_case_backend_parity(SUITE, "complex_clocks", expected, &[], &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "complex_clocks",
        expected,
        &["--model-opt-level", "O0"],
        &[],
    );
    // Clocking-event lists come from owned Db capture only.
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join("complex_clocks.sv");
    sim_cli::run_compile_opts_after_db_drop(
        SUITE,
        "complex_clocks",
        llg::core::compile::CompileOpts {
            files: vec![source.to_string_lossy().into_owned()],
            top: Some("tb".to_owned()),
            ..Default::default()
        },
        expected,
        "INFO: complex_clocks.sv:85:8: [SIM-001] (tb @ 17 ns) $finish called\n",
    );
}

#[test]
fn overlapping_assertions_and_actions_share_sampled_histories() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_035/assertion_history.out");
    sim_cli::run_case_backend_parity(SUITE, "assertion_history", expected, &[], &[]);
}

#[test]
fn event_clock_gates_are_read_when_the_event_occurs() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_035/event_gate_instant.out");
    sim_cli::run_case_backend_parity(SUITE, "event_gate_instant", expected, &[], &[]);
}

// FND-002 witnesses, adopted with a reviewed oracle.

#[test]
fn expression_clock_witness() {
    // L-F12-11-02: `$past` on an expression edge clock. The draft expected 0;
    // with no strictly earlier tick 16.9.3 returns the initial value of `x`.
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_035/expression_clock_witness.out");
    sim_cli::run_case_backend_parity(SUITE, "expression_clock_witness", expected, &[], &[]);
}

#[test]
fn real_sample_witness() {
    // L-F12-11-03: `$sampled` of a real keeps its numeric Preponed value.
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_035/real_sample_witness.out");
    sim_cli::run_case_backend_parity(SUITE, "real_sample_witness", expected, &[], &[]);
}

// Language-illegal forms are diagnosed, never coerced.

#[test]
fn neg_rose_real() {
    sim_cli::reject_case(
        SUITE,
        "neg_rose_real",
        "$rose of a real expression is illegal",
    );
}

#[test]
fn neg_fell_shortreal() {
    sim_cli::reject_case(
        SUITE,
        "neg_fell_shortreal",
        "$fell of a real expression is illegal",
    );
}

#[test]
fn neg_real_assertion() {
    sim_cli::reject_case(
        SUITE,
        "neg_real_assertion",
        "$changed of a real expression is illegal",
    );
}

#[test]
fn neg_real_past() {
    sim_cli::reject_case(
        SUITE,
        "neg_real_past",
        "$past of a real expression is illegal",
    );
}

#[test]
fn neg_stable_realtime() {
    sim_cli::reject_case(
        SUITE,
        "neg_stable_realtime",
        "$stable of a real expression is illegal",
    );
}

#[test]
fn neg_string_arg() {
    sim_cli::reject_case(
        SUITE,
        "neg_string_arg",
        "$stable argument of type `string` is illegal",
    );
}

#[test]
fn neg_sampled_string() {
    sim_cli::reject_case(
        SUITE,
        "neg_sampled_string",
        "$sampled of a string expression is illegal",
    );
}

#[test]
fn neg_class_arg() {
    sim_cli::reject_case(
        SUITE,
        "neg_class_arg",
        "$changed argument of type `C` is illegal",
    );
}

#[test]
fn neg_event_arg() {
    sim_cli::reject_case(
        SUITE,
        "neg_event_arg",
        "$changed argument of type `event` is illegal",
    );
}

#[test]
fn neg_dynamic_arg() {
    sim_cli::reject_case(
        SUITE,
        "neg_dynamic_arg",
        "exclude dynamic and associative array operands",
    );
}

#[test]
fn neg_assoc_arg() {
    sim_cli::reject_case(
        SUITE,
        "neg_assoc_arg",
        "exclude dynamic and associative array operands",
    );
}

#[test]
fn neg_automatic_arg() {
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_arg",
        "variables in sampled-value arguments shall be static",
    );
}

#[test]
fn neg_past_zero() {
    sim_cli::reject_case(
        SUITE,
        "neg_past_zero",
        "'number_of_ticks' argument must be greater than or equal to 1",
    );
}

#[test]
fn neg_past_negative() {
    sim_cli::reject_case(
        SUITE,
        "neg_past_negative",
        "'number_of_ticks' argument must be greater than or equal to 1",
    );
}

#[test]
fn neg_past_nonconst() {
    sim_cli::reject_case(
        SUITE,
        "neg_past_nonconst",
        "reference to non-constant variable 'n' is not allowed in a constant expression",
    );
}

#[test]
fn neg_past_unknown_ticks() {
    sim_cli::reject_case(
        SUITE,
        "neg_past_unknown_ticks",
        "value must not have any unknown bits",
    );
}

#[test]
fn neg_no_clock() {
    sim_cli::reject_case(
        SUITE,
        "neg_no_clock",
        "could not determine clocking event for '$rose'",
    );
}

// Legal forms outside this task stay explicit rejections.

#[test]
fn neg_unpacked_arg() {
    sim_cli::reject_case(
        SUITE,
        "neg_unpacked_arg",
        "is not supported: sampled histories hold packed integral and real values, not unpacked arrays",
    );
}

#[test]
fn neg_unpacked_struct_arg() {
    sim_cli::reject_case(
        SUITE,
        "neg_unpacked_struct_arg",
        "is not supported: sampled histories hold packed integral and real values, not unpacked structures",
    );
}

#[test]
fn neg_queue_arg() {
    sim_cli::reject_case(
        SUITE,
        "neg_queue_arg",
        "is not supported: sampled histories hold packed integral and real values, not queues",
    );
}

// sampled-array-cells: cells of fixed unpacked arrays below the descriptor
// size are sampled with their indices (SV 16.5, 16.6.1); container elements
// are rejected at the selection.

#[test]
fn element_histories_sample_the_cell_and_its_index() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_035/element_past.out");
    sim_cli::run_case_backend_parity(SUITE, "element_past", expected, &[], &[]);
}

#[test]
fn element_assertion_operands_read_sampled_cells() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_035/element_assertion.out");
    sim_cli::run_case_backend_parity(SUITE, "element_assertion", expected, &[], &[]);
}

#[test]
fn element_selections_and_multidimensional_cells_are_sampled() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_035/element_selects.out");
    sim_cli::run_case_backend_parity(SUITE, "element_selects", expected, &[], &[]);
}

#[test]
fn real_net_enum_and_structure_cells_are_sampled() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_035/element_types.out");
    sim_cli::run_case_backend_parity(SUITE, "element_types", expected, &[], &[]);
}

/// Descriptor-form cells (above 4096) keep saved Preponed values per slot: a
/// burst that changes every cell, then single writes, reads no stale value.
#[test]
fn descriptor_cells_sampled_after_a_burst_of_writes() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_035/element_burst.out");
    sim_cli::run_case_backend_parity(SUITE, "element_burst", expected, &[], &[]);
}

/// Only arrays a sampled expression selects keep a Preponed image, and it is
/// registered before the history domains whose initial values read it.
#[test]
fn component_only_sampled_arrays_keep_a_cell_image() {
    for optimized in [true, false] {
        let (directory, output) =
            sim_cli::run_case_keeping_dir(SUITE, "element_past", optimized, &["--gen-only"], &[]);
        assert!(output.status.success(), "{output:?}");
        let model = std::fs::read_to_string(directory.path().join("build/sim/tb/model.c"))
            .expect("generated model");
        assert_eq!(model.matches("llg_sampled_cells(").count(), 1, "{model}");
        assert!(!model.contains("G_tb_plain_llg_sampled"), "{model}");
        let image = model
            .find("G_tb_arr_llg_sampled = llg_sampled_cells(G_tb_arr, 4ULL);")
            .expect("image registration");
        let domain = model
            .find("llg_sampled_domain_register(")
            .expect("domain registration");
        assert!(image < domain, "{model}");
    }
}

#[test]
fn neg_queue_element() {
    sim_cli::reject_case(
        SUITE,
        "neg_queue_element",
        "neg_queue_element.sv:10:47 $past argument selecting a container element in `tb` is not \
         supported: llg keeps no sampled value of queue, dynamic array or associative array \
         elements",
    );
}

#[test]
fn neg_dynamic_element_operand() {
    sim_cli::reject_case(
        SUITE,
        "neg_dynamic_element_operand",
        "neg_dynamic_element_operand.sv:9:39 unsupported sampled consequent expression in \
         concurrent assertion (tb): llg keeps no sampled value of queue, dynamic array or \
         associative array elements",
    );
}

#[test]
fn neg_assoc_element() {
    sim_cli::reject_case(
        SUITE,
        "neg_assoc_element",
        "neg_assoc_element.sv:9:47 $past argument selecting a container element in `tb`",
    );
}

#[test]
fn neg_real_element_past() {
    sim_cli::reject_case(
        SUITE,
        "neg_real_element_past",
        "neg_real_element_past.sv:11:42 $past of a real expression is illegal",
    );
}
