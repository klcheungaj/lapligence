//! G1-21 procedural control-flow acceptance through the public HDL CLI.
//!
//! Positive fixtures run in both optimizer modes with exact stdout. The
//! deferred pattern-binding form is pinned as a rejection so it can never be
//! silently executed as an ordinary case before G3-01.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

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
        source.display()
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
fn pattern_binding_case_is_rejected_until_g3() {
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "feature_completion/g1_21",
            "pattern_case",
            optimized,
            &[],
            &[],
            &[],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !output.status.success(),
            "optimized={optimized}: pattern case must be rejected: {stderr}"
        );
        assert!(
            output.stdout.is_empty(),
            "optimized={optimized}: pattern case produced stdout: {output:?}"
        );
    }
}
