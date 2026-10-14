//! SIM-029: constrained randomization (IEEE 1800-2009 clause 18). Positive
//! fixtures print relations that hold for every legal solution, so their
//! expected output does not depend on the values drawn; the readme quotes the
//! clauses each line checks. Negative fixtures name every construct outside
//! the SIM-029 scope: none may be silently left unconstrained.
use super::sim_cli;
use std::process::Output;

const SUITE: &str = "feature_completion/sim_029";

/// Budgets that leave the solver no work: every satisfiable problem that
/// needs a search ends with RESOURCE.
const NO_BUDGET: [(&str, &str); 3] = [
    ("LLG_RAND_BDD_NODES", "1"),
    ("LLG_RAND_REJECT_TRIES", "0"),
    ("LLG_RAND_CONFLICTS", "0"),
];

#[test]
fn class_constraints_inheritance_and_static_members() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "class_constraints",
        include_str!("../fixtures/sim/feature_completion/sim_029/class_constraints.out"),
        &[],
        &[],
    );
}

#[test]
fn rand_mode_and_constraint_mode() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "modes",
        include_str!("../fixtures/sim/feature_completion/sim_029/modes.out"),
        &[],
        &[],
    );
}

#[test]
fn failed_randomize_changes_nothing_and_hooks_run_in_order() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "unsat_and_hooks",
        include_str!("../fixtures/sim/feature_completion/sim_029/unsat_and_hooks.out"),
        &[],
        &[],
    );
}

#[test]
fn inline_constraints_variable_lists_and_scope_randomize() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "inline_and_lists",
        include_str!("../fixtures/sim/feature_completion/sim_029/inline_and_lists.out"),
        &[],
        &[],
    );
}

#[test]
fn signed_ranges_are_exact() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "signed_ranges",
        include_str!("../fixtures/sim/feature_completion/sim_029/signed_ranges.out"),
        &[],
        &[],
    );
}

#[test]
fn constraint_functions_and_guards() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "functions_and_guards",
        include_str!("../fixtures/sim/feature_completion/sim_029/functions_and_guards.out"),
        &[],
        &[],
    );
}

/// A located rejection names the fixture's source position, not only the
/// construct.
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
fn sim_030_constructs_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_randc",
        "`randc` property `r` of class `c_c` is not supported (planned for SIM-030) at ",
    );
    names_source_position("neg_randc");
    sim_cli::reject_case(
        SUITE,
        "neg_dist",
        "unsupported constraint expression (`dist` is planned for SIM-030) at ",
    );
    names_source_position("neg_dist");
    sim_cli::reject_case(
        SUITE,
        "neg_solve_before",
        "`solve ... before` is not supported (planned for SIM-030) at ",
    );
    names_source_position("neg_solve_before");
    sim_cli::reject_case(
        SUITE,
        "neg_foreach",
        "`foreach` constraints are not supported (planned for SIM-030) at ",
    );
    names_source_position("neg_foreach");
}

#[test]
fn unsupported_random_variables_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_rand_array",
        "random property `v` of class `c_c` is not a packed integral value",
    );
    names_source_position("neg_rand_array");
    sim_cli::reject_case(
        SUITE,
        "neg_rand_handle",
        "random property `l` of class `c_c` is not a packed integral value",
    );
    names_source_position("neg_rand_handle");
    sim_cli::reject_case(
        SUITE,
        "neg_enum_struct",
        "random property `s` of class `c_c` is not a packed integral value",
    );
    names_source_position("neg_enum_struct");
}

#[test]
fn unsupported_constraint_operators_are_located_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_random_divisor",
        "a random divisor in `/` or `%` is not supported",
    );
    names_source_position("neg_random_divisor");
    sim_cli::reject_case(
        SUITE,
        "neg_random_index",
        "a select whose index is a random variable is not supported (S29-D7)",
    );
    names_source_position("neg_random_index");
    sim_cli::reject_case(
        SUITE,
        "neg_pow",
        "`**` is not supported on random variables in constraints",
    );
    names_source_position("neg_pow");
}

#[test]
fn value_randomize_outside_one_evaluation_is_a_located_error() {
    sim_cli::reject_case(
        SUITE,
        "neg_short_circuit",
        "a randomize call in a short-circuit or conditional operand is not supported",
    );
    names_source_position("neg_short_circuit");
    sim_cli::reject_case(
        SUITE,
        "neg_loop_condition",
        "a randomize call used as a value is supported only in",
    );
    names_source_position("neg_loop_condition");
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
fn null_receiver_is_a_runtime_error() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "rt_null_receiver",
        &[],
        &runtime_failure(
            "before\n",
            "null class handle access: randomize() at tb:8:10",
        ),
    );
}

#[test]
fn unguarded_null_reference_fails_randomize() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "rt_unguarded_null",
        &[],
        &runtime_failure(
            "ok=0 v=5\n",
            "llg: error: randomize() at tb:14:10: null object handle in a constraint",
        ),
    );
}

#[test]
fn circular_function_ordering_fails_randomize() {
    sim_cli::run_case_checked_matrix(
        SUITE,
        "rt_circular_order",
        &[],
        &runtime_failure(
            "ok=0\n",
            "llg: error: randomize() at tb:12:10: circular function-argument order (`x`)",
        ),
    );
}

/// A03: an exhausted budget is a run-time error naming the budget, distinct
/// from the warning of an unsatisfiable problem.
#[test]
fn exhausted_budget_is_distinct_from_unsat() {
    let failure = runtime_failure(
        "unsat ok=0\nresource ok=0 x=0 y=0\n",
        "llg: error: randomize() at tb:18:10: solver budget",
    );
    sim_cli::run_case_checked_matrix_with_env(
        SUITE,
        "rt_resource",
        &[],
        &NO_BUDGET,
        &|label: &str, output: &Output| {
            failure(label, output);
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                stderr.contains(
                    "llg: warning: randomize() at tb:16:10: the constraints cannot be satisfied; randomize returns 0"
                ),
                "{label}: {stderr}"
            );
        },
    );
}
