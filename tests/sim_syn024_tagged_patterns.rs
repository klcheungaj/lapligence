//! SYN-024: finite tagged patterns through the public simulator.
use crate::sim_cli;

#[test]
fn tagged_bindings_nested_payloads_and_conditional_arms() {
    sim_cli::run_case_with_args(
        "syn024_tagged_patterns",
        "tagged_runtime",
        "tagged_runtime=pass checks=10 calls=1\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn tagged_pattern_tags_follow_the_enclosing_case_mode() {
    sim_cli::run_case_with_args(
        "syn024_tagged_patterns",
        "case_modes",
        "tagged_case_modes=pass checks=8 calls=4\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn tagged_patterns_require_systemverilog() {
    sim_cli::run_case_with_args(
        "syn024_tagged_patterns",
        "edition_boundary",
        "tagged_edition=pass\n",
        "",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        "syn024_tagged_patterns",
        "edition_boundary",
        "undeclared identifier 'matches'",
        &["--edition", "v2001"],
    );
}

#[test]
fn wrong_tag_and_source_type_are_rejected() {
    for (fixture, diagnostic) in [("bad_tag_name", "missing"), ("bad_source_type", "tagged")] {
        sim_cli::reject_case_with_args(
            "syn024_tagged_patterns",
            fixture,
            diagnostic,
            &["--edition", "sv2009"],
        );
    }
}
