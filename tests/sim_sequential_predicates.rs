//! R06: source-ordered predicates in independent optimized and unoptimized CLI runs.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn sequential_predicate_truth_table() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "truth_table", "truth_table=64 true=1 false=21 unknown=42\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_branch_roles() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "branch_roles", "branch_roles=pass\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_effects() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "effects", "effects=6 cases passed\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_constant_prefix() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "constant_prefix", "constant_prefix=pass calls=2\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_wide_truth() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "wide_truth", "wide_truth=pass\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_real_result() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "real_result", "real_result=pass calls=3,3\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_array_results() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "array_results", "array_results=pass\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_clock_sensitivity() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "clock_sensitivity", "clock_sensitivity=pass\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_nested_qualifiers() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "nested_qualifiers", "nested_qualifiers=pass\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_reduction_context() {
    sim_cli::run_case_with_args(
        "sequential_predicates", "reduction_context", "reduction_context=pass\n", "", &[],
        &["--edition", "2009"],
    );
}

#[test]
fn syn_022_basic_patterns() {
    sim_cli::run_case_with_args(
        "sequential_predicates",
        "syn_022_basic_patterns",
        "patterns=pass result=3c calls=1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn syn_023_recursive_structure_patterns() {
    sim_cli::run_case_with_args(
        "sequential_predicates",
        "syn_023_structure_patterns",
        "structure_patterns=pass checks=6 calls=1\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_bad_matches_if() {
    sim_cli::reject_case_with_args(
        "sequential_predicates", "bad_matches_if", "pattern",
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_bad_matches_conditional() {
    sim_cli::reject_case_with_args(
        "sequential_predicates", "bad_matches_conditional", "too few",
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_bad_matches_incompatible() {
    sim_cli::reject_case_with_args(
        "sequential_predicates", "bad_matches_incompatible", "not a struct",
        &["--edition", "2009"],
    );
}

#[test]
fn sequential_predicate_bad_matches_out_of_scope() {
    sim_cli::reject_case_with_args(
        "sequential_predicates",
        "bad_matches_out_of_scope",
        "undeclared identifier",
        &["--edition", "2009"],
    );
}
