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
        "llg: $finish at time 17000 at tb:91:8\n",
    );
}

#[test]
fn overlapping_assertions_and_actions_share_sampled_histories() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_035/assertion_history.out");
    sim_cli::run_case_backend_parity(SUITE, "assertion_history", expected, &[], &[]);
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

#[test]
fn neg_future_global() {
    // ADV-014 owns the global-clock future functions (SV 16.9.4).
    sim_cli::reject_case(
        SUITE,
        "neg_future_global",
        "future global sampled-value function `$future_gclk` is not supported",
    );
}
