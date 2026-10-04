//! IEEE 1800-2009 12.6.1 pattern cases and 12.5.3 qualifier diagnostics.
#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn qualifier_counts_filtered_items_and_preserves_first_body() {
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/syn025_pattern_cases/qualifiers.sv");
    let warning = |line, qualifier, reason| {
        format!(
            "{qualifier} violation at {}:{line}:9: {reason}",
            sim_harness::source_display(&source)
        )
    };
    let warnings = [
        warning(16, "unique", "multiple matching items"),
        warning(23, "unique", "multiple matching items"),
        warning(30, "unique0", "multiple matching items"),
        warning(37, "unique", "no matching item"),
        warning(47, "priority", "no matching item"),
    ];
    sim_cli::run_case_with_args(
        "syn025_pattern_cases",
        "qualifiers",
        "qualifiers=pass result=0 calls=1\n",
        "",
        &warnings.iter().map(String::as_str).collect::<Vec<_>>(),
        &["--edition", "2009"],
    );
}

#[test]
fn ordinary_case_controls_keep_their_match_rules() {
    sim_cli::run_case_with_args(
        "syn025_pattern_cases",
        "ordinary_controls",
        "ordinary_controls=pass\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn runtime_selectors_and_pattern_side_wildcards_follow_case_mode() {
    sim_cli::run_case_with_args(
        "syn025_pattern_cases",
        "runtime_modes",
        "runtime_modes=pass checks=21 calls=8\n",
        "",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn pattern_case_rejects_2001_edition() {
    sim_cli::reject_case_with_args(
        "syn025_pattern_cases",
        "ordinary_controls",
        "undeclared identifier 'matches'",
        &["--edition", "2001"],
    );
}

#[test]
fn pattern_binding_cannot_escape_its_item() {
    sim_cli::reject_case_with_args(
        "syn025_pattern_cases",
        "bad_binding_scope",
        "undeclared identifier 'local_value'",
        &["--edition", "2009"],
    );
}

#[test]
fn dynamic_pattern_source_diagnoses() {
    sim_cli::reject_case_with_exact_stderr(
        "syn025_pattern_cases",
        "bad_dynamic_pattern",
        "llg: codegen error: string/class signals are not supported: `bound` in `tb`\n",
        &["--edition", "2009"],
    );
}
