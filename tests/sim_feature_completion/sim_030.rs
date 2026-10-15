//! SIM-030: `dist`, `solve...before`, `randc`, random divisors, select
//! indices and `**`, random arrays, array methods, `foreach` and random
//! object members (IEEE 1800-2009 18.4, 18.5.4, 18.5.7-18.5.9, 18.6.2). Positive
//! fixtures print relations every legal solution satisfies and frequencies
//! checked inside the simulation against bounds fixed in advance (five
//! standard deviations), so the expected output does not depend on the
//! values drawn; the readme quotes the clauses each line checks.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_030";

#[test]
fn dist_weights_partition_by_density() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "dist_weights",
        include_str!("../fixtures/sim/feature_completion/sim_030/dist_weights.out"),
        &[],
        &[],
    );
}

#[test]
fn solve_before_orders_the_draws() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "solve_before",
        include_str!("../fixtures/sim/feature_completion/sim_030/solve_before.out"),
        &[],
        &[],
    );
}

#[test]
fn randc_cycles_per_object_and_restarts_on_constraint_changes() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "randc",
        include_str!("../fixtures/sim/feature_completion/sim_030/randc.out"),
        &[],
        &[],
    );
}

#[test]
fn random_divisors_indices_and_power_exclude_erroneous_assignments() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "random_ops",
        include_str!("../fixtures/sim/feature_completion/sim_030/random_ops.out"),
        &[],
        &[],
    );
}

#[test]
fn inheritance_and_inline_shadowing_select_the_right_fields() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "shadowing",
        include_str!("../fixtures/sim/feature_completion/sim_030/shadowing.out"),
        &[],
        &[],
    );
}

#[test]
fn random_arrays_size_first_and_keep_unconstrained_sizes() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "rand_arrays",
        include_str!("../fixtures/sim/feature_completion/sim_030/rand_arrays.out"),
        &[],
        &[],
    );
}

#[test]
fn array_reductions_inside_and_random_indices() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "array_methods",
        include_str!("../fixtures/sim/feature_completion/sim_030/array_methods.out"),
        &[],
        &[],
    );
}

#[test]
fn random_object_members_are_solved_with_their_parent() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "rand_objects",
        include_str!("../fixtures/sim/feature_completion/sim_030/rand_objects.out"),
        &[],
        &[],
    );
}

#[test]
fn std_randomize_solves_scope_arrays() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "scope_arrays",
        include_str!("../fixtures/sim/feature_completion/sim_030/scope_arrays.out"),
        &[],
        &[],
    );
}

/// Storage of randomize-only targets and constraint-only state survives the
/// optimizer's unused-storage pass.
#[test]
fn randomize_only_targets_keep_storage() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "randomize_only_targets",
        include_str!("../fixtures/sim/feature_completion/sim_030/randomize_only_targets.out"),
        &[],
        &[],
    );
}

/// A03: every 2009 constraint production executes.
#[test]
fn every_2009_constraint_production_has_a_witness() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "productions",
        include_str!("../fixtures/sim/feature_completion/sim_030/productions.out"),
        &[],
        &[],
    );
}

/// A03: later-edition constraint syntax stays rejected under sv2009.
#[test]
fn later_edition_constraints_are_rejected() {
    sim_cli::reject_case(SUITE, "neg_soft", "expected ';'");
    names_source_position("neg_soft");
    sim_cli::reject_case(
        SUITE,
        "neg_unique",
        "`uniqueness constraint` is not available in IEEE 2009",
    );
    names_source_position("neg_unique");
}

#[test]
fn unsupported_array_and_object_forms_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_array_compare",
        "a random array may be used in a constraint only through element selects",
    );
    names_source_position("neg_array_compare");
    sim_cli::reject_case(
        SUITE,
        "neg_handle_array",
        "array `l` is not supported in constraints: arrays of class handles are not supported",
    );
    names_source_position("neg_handle_array");
    sim_cli::reject_case(
        SUITE,
        "neg_foreach_2d",
        "array `m` is not supported in constraints: it has more than one unpacked dimension",
    );
    names_source_position("neg_foreach_2d");
    sim_cli::reject_case(
        SUITE,
        "neg_element_call",
        "a function call argument that reads an array element, array size, loop index or another object's variable is not supported",
    );
    names_source_position("neg_element_call");
}

/// A located rejection names the fixture's source position.
fn names_source_position(fixture: &str) {
    let source = format!("{fixture}.sv:");
    let output = sim_cli::invoke_with_env(SUITE, fixture, false, &[], &[], &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&source),
        "{fixture}: the diagnostic names the source position: {stderr}"
    );
}

#[test]
fn unsupported_and_later_edition_forms_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_randc_wide",
        "`randc` property `r` of class `c_c` must be a packed integral value of at most 16 bits",
    );
    names_source_position("neg_randc_wide");
    sim_cli::reject_case(
        SUITE,
        "neg_dist_random_weight",
        "a dist item value, range bound or weight that reads a random variable is not supported (S30-D10)",
    );
    names_source_position("neg_dist_random_weight");
    sim_cli::reject_case(
        SUITE,
        "neg_dist_default",
        "language feature is not supported",
    );
    names_source_position("neg_dist_default");
}

/// A run-time failure: exact stdout, exit status 1 and `diagnostic` on
/// stderr.
fn runtime_failure<'a>(stdout: &'a str, diagnostic: &'a str) -> impl Fn(&str, &Output) + 'a {
    move |label: &str, output: &Output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
        assert!(stderr.contains(diagnostic), "{label}: {stderr}");
    }
}

#[test]
fn negative_dist_weight_fails_randomize() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "rt_negative_weight",
        &[],
        &runtime_failure(
            "ok=0 x=7\n",
            "llg: error: randomize() at tb:12:10: negative or unevaluated dist weight",
        ),
    );
}

#[test]
fn circular_solve_before_fails_only_while_both_blocks_are_enabled() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "rt_circular_solve_before",
        &[],
        &runtime_failure(
            "ok=0\nok=1\n",
            "llg: error: randomize() at tb:11:10: circular solve-before order",
        ),
    );
}

#[test]
fn foreach_index_outside_the_array_fails_randomize() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "rt_foreach_index",
        &[],
        &runtime_failure(
            "ok=0 size=0\nok=1 size=3\n",
            "llg: error: randomize() at tb:13:10: select index out of range in a constraint",
        ),
    );
}
