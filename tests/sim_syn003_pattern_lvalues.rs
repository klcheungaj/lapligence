//! SYN-003 positional assignment-pattern lvalues through the public CLI.
//!
//! The legal fixture covers plain and typedef-qualified patterns, nested
//! positions, selected fixed-array cells, reversed source bounds, overlap
//! capture, a pattern RHS, and blocking/NBA publication in both optimizer
//! modes. Negative fixtures keep keyed/default, replicated, malformed-shape,
//! constant-target, and per-element width failures independent.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn positional_pattern_lvalues_run_in_both_optimizer_modes() {
    sim_cli::run_case(
        "syn003_pattern_lvalues",
        "syn_003_pattern_lvalues",
        "plain 11 22\ntyped 22 11\npattern_rhs 23 24\nselected 11 22\nreversed 31 42\nnested 51 52 61 62\noverlap 82\nnba_before 00 00\nnba_after 11 22\nPASS syn_003_pattern_lvalues\n",
        "llg: $finish at time 1000 at tb:56:5\n",
        &[],
    );
}

#[test]
fn keyed_pattern_lvalue_is_rejected() {
    sim_cli::reject_case(
        "syn003_pattern_lvalues",
        "syn_003_keyed_lvalue",
        "expression is not assignable",
    );
}

#[test]
fn replicated_pattern_lvalue_is_rejected() {
    sim_cli::reject_case(
        "syn003_pattern_lvalues",
        "syn_003_replicated_lvalue",
        "expression is not assignable",
    );
}

#[test]
fn pattern_lvalue_width_mismatch_is_rejected() {
    sim_cli::reject_case(
        "syn003_pattern_lvalues",
        "syn_003_width_mismatch",
        "assignment-pattern lvalue target",
    );
}

#[test]
fn constant_pattern_lvalue_is_rejected() {
    sim_cli::reject_case(
        "syn003_pattern_lvalues",
        "syn_003_constant_lvalue",
        "expression is not assignable",
    );
}

#[test]
fn pattern_lvalue_shape_mismatch_is_rejected() {
    sim_cli::reject_case(
        "syn003_pattern_lvalues",
        "syn_003_shape_mismatch",
        "requires 2 elements",
    );
}

#[test]
fn automatic_pattern_lvalue_nba_is_rejected() {
    sim_cli::reject_case(
        "syn003_pattern_lvalues",
        "syn_003_automatic_nba",
        "automatic assignment-pattern target",
    );
}

#[test]
fn ref_pattern_lvalue_nba_is_rejected() {
    sim_cli::reject_case(
        "syn003_pattern_lvalues",
        "syn_003_ref_nba",
        "automatic assignment-pattern target",
    );
}

#[test]
fn static_ref_pattern_lvalue_nba_is_rejected() {
    sim_cli::reject_case(
        "syn003_pattern_lvalues",
        "syn_003_static_ref_nba",
        "'ref' arguments can only be used in 'automatic' subroutines",
    );
}
