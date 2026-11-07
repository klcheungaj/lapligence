//! Focused public-CLI regressions for assignment-pattern keys and lvalues.
use crate::sim_cli;

const SUITE: &str = "review_bundle";
const EDITION: &[&str] = &["--edition", "sv2009"];

#[test]
fn fixed_array_type_keys_obey_last_match_recursion_and_default_coverage() {
    for (fixture, pass_line, finish_line) in [
        (
            "r04_duplicate_type_keys",
            "PASS r04_duplicate_type_keys\n",
            10,
        ),
        (
            "r04_recursive_type_keys",
            "PASS r04_recursive_type_keys\n",
            13,
        ),
        (
            "r04_unmatched_type_with_default",
            "PASS r04_unmatched_type_with_default\n",
            12,
        ),
    ] {
        let expected_stderr =
            format!("INFO: {fixture}.sv:{finish_line}:5: [SIM-001] (tb @ 0 ps) $finish called\n");
        sim_cli::run_case_with_args(SUITE, fixture, pass_line, &expected_stderr, &[], EDITION);
    }
}

#[test]
fn constant_expression_array_index_keys_are_evaluated_semantically() {
    sim_cli::run_case_with_args(
        SUITE,
        "r05_constant_expression_index",
        "PASS r05_constant_expression_index\n",
        "INFO: r05_constant_expression_index.sv:21:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        EDITION,
    );
}

#[test]
fn typed_struct_and_packed_array_pattern_lvalues_deconstruct_positions() {
    for (fixture, pass_line, finish_line) in [
        (
            "r06_packed_struct_pattern_lvalue",
            "PASS r06_packed_struct_pattern_lvalue\n",
            12,
        ),
        (
            "r06_unpacked_struct_pattern_lvalue",
            "PASS r06_unpacked_struct_pattern_lvalue\n",
            12,
        ),
        (
            "r06_packed_array_pattern_lvalue",
            "PASS r06_packed_array_pattern_lvalue\n",
            12,
        ),
    ] {
        let expected_stderr =
            format!("INFO: {fixture}.sv:{finish_line}:5: [SIM-001] (tb @ 0 ps) $finish called\n");
        sim_cli::run_case_with_args(SUITE, fixture, pass_line, &expected_stderr, &[], EDITION);
    }
}
