//! Phase 01 P40 gate-terminal acceptance tests.
//!
//! IEEE 1364-2001 7.1.5-7.4 and IEEE 1800-2009 28.3-28.6 cover primitive
//! instance arrays, terminal ordering, buffer outputs, and four-state values.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn gate_terminal_forms_match_with_and_without_optimization() {
    sim_cli::run_case(
        "gates",
        "p40_forms",
        "array=1000 selected=1 expression=0 mixed=0\n\
buf=00 not=00 hierarchy=00 many=1\n\
wake=000\n\
hierwake=11 xz=xx\n",
        "llg: $finish at time 3000 at tb:62:9\n",
        &[],
    );
}

#[test]
fn gate_strength_resolution_is_consistent_in_both_modes() {
    sim_cli::run_case(
        "gates",
        "p40_strength_rejected",
        "PASS gate_strength\n",
        "",
        &[],
    );
}

#[test]
fn gate_arrays_in_module_and_generate_scopes_keep_each_driver() {
    sim_cli::run_case(
        "gates",
        "array_scopes",
        "direct=1000 generated=1110\ndirect=0100 generated=1101\n",
        "",
        &[],
    );
}
