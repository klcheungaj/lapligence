//! G1-21 procedural control-flow acceptance through the public HDL CLI.
//!
//! Positive fixtures run in both optimizer modes with exact stdout, including
//! tagged pattern-case selection through the owned tag guard.

use crate::sim_cli;
use crate::sim_harness;

#[test]
fn control_nested_foreach_cleanup() {
    sim_cli::run_case(
        "feature_completion/g1_21",
        "nested_foreach_cleanup",
        "skip acc=130\nnest acc=1193 ii=12\n",
        "",
        &[],
    );
}

#[test]
fn case_four_state_checks() {
    sim_cli::run_case(
        "feature_completion/g1_21",
        "case_four_state",
        "a 1 2 3 0\nb 0 0 3 5\nc 0 2 0 4\n",
        "",
        &[],
    );
}

#[test]
fn qualified_string_case_inside_keeps_first_match_and_qualifiers() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/g1_21/qualified_string_inside.sv");
    let warning = format!(
        "unique violation at {}:32:9: no matching item",
        sim_harness::source_display(&source)
    );
    sim_cli::run_case(
        "feature_completion/g1_21",
        "qualified_string_inside",
        "a o=1\nb o=2\nc o=2\nd o=2\n",
        "",
        &[warning.as_str()],
    );
}

#[test]
fn loop_not_synthesis_proven() {
    sim_cli::run_case(
        "feature_completion/g1_21",
        "runtime_loop",
        "sum=15 i=6\n",
        "",
        &[],
    );
}

#[test]
fn tagged_pattern_case_selects_only_the_active_arm() {
    sim_cli::run_case("feature_completion/g1_21", "pattern_case", "o=1\n", "", &[]);
}
