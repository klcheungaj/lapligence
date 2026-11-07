//! SYN-017: selected directive and lexical effects through the public simulator.

use crate::sim_cli;

use std::path::Path;

const SUITE: &str = "syn017_directives";

#[test]
fn legacy_parameter_macros_conditionals_and_generated_ranges_execute() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "legacy_macro",
            "value=0000101\n",
            "INFO: legacy_macro.sv:18:5: [SIM-001] (tb @ 0 ps) $finish called\n",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn legacy_ifdef_elsif_else_selects_each_executable_branch() {
    for edition in ["v2001", "sv2009"] {
        for (define, expected) in [
            (Some("ENABLE"), "choice=11\n"),
            (Some("ALT"), "choice=22\n"),
            (None, "choice=33\n"),
        ] {
            let mut args = vec!["--edition", edition];
            if let Some(define) = define {
                args.extend(["--define", define]);
            }
            sim_cli::run_case_with_args(
                SUITE,
                "legacy_conditions",
                expected,
                "INFO: legacy_conditions.sv:12:5: [SIM-001] (tb @ 0 ps) $finish called\n",
                &[],
                &args,
            );
        }
    }
}

#[test]
fn modern_macro_operators_execute_only_in_2009() {
    sim_cli::run_case_with_args(
        SUITE,
        "modern_macro",
        "name=value_field value=37\n",
        "INFO: modern_macro.sv:11:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "modern_macro",
        "not available in IEEE 2001",
        &["--edition", "v2001"],
    );
}

#[test]
fn implicit_net_policy_follows_compilation_unit_boundary() {
    for edition in ["v2001", "sv2009"] {
        for (prefix, mode) in [
            ("nettype_def", "separate"),
            ("nettype_wire", "merged"),
            ("nettype_reset", "merged"),
        ] {
            sim_cli::run_case_with_source_prefix(
                SUITE,
                "nettype_use",
                &[prefix],
                "implicit=z\n",
                "INFO: nettype_use.sv:7:5: [SIM-001] (tb @ 0 ps) $finish called\n",
                &[],
                &["--edition", edition, "--compilation-units", mode],
            );
        }
        for optimized in [false, true] {
            let output = sim_cli::invoke_with_source_prefix(
                SUITE,
                "nettype_use",
                &["nettype_def"],
                optimized,
                &["--edition", edition, "--compilation-units", "merged"],
            );
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert!(output.stdout.is_empty(), "{output:?}");
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("use of undeclared identifier 'implicit_wire'"),
                "{output:?}"
            );
        }
    }
}

#[test]
fn later_directives_and_predefined_macros_are_edition_gated() {
    for (fixture, expected, finish_line, diagnostic) in [
        ("later_paste", "paste=1\n", 8, "macro token paste"),
        (
            "later_stringify",
            "quote=value\n",
            6,
            "macro stringification",
        ),
        ("later_undefineall", "undefineall=1\n", 12, "`undefineall`"),
        ("later_pragma", "pragma=1\n", 8, "`pragma`"),
        ("later_keywords", "keywords=1\n", 8, "`begin_keywords`"),
    ] {
        sim_cli::run_case_with_args(
            SUITE,
            fixture,
            expected,
            &format!("INFO: {fixture}.sv:{finish_line}:5: [SIM-001] (tb @ 0 ps) $finish called\n"),
            &[],
            &["--edition", "sv2009"],
        );
        sim_cli::reject_case_with_args(SUITE, fixture, diagnostic, &["--edition", "v2001"]);
    }
    sim_cli::reject_case_with_args(
        "directive_effects",
        "line_mapping",
        "`__FILE__`",
        &["--edition", "v2001"],
    );
    sim_cli::reject_case_with_args(
        "directive_effects",
        "macros_include",
        "macro token paste",
        &["--edition", "v2001"],
    );
}

#[test]
fn later_forms_in_untaken_ifdef_are_ignored() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "inactive_later_forms",
            "ok\n",
            "INFO: inactive_later_forms.sv:10:5: [SIM-001] (tb @ 0 ps) $finish called\n",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn later_directive_in_else_follows_selected_branch() {
    sim_cli::run_case_with_args(
        SUITE,
        "else_directive_activity",
        "if\n",
        "INFO: else_directive_activity.sv:6:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        &["--edition", "v2001", "--define", "TAKE_IF"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "else_directive_activity",
        "`pragma`",
        &["--edition", "v2001"],
    );
    sim_cli::run_case_with_args(
        SUITE,
        "else_directive_activity",
        "else\n",
        "INFO: else_directive_activity.sv:14:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn caller_header_precedes_later_include_directory() {
    let later =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/sim/syn017_directives/later");
    let later = later.to_str().expect("fixture directory is UTF-8");
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "include_order",
            "choice=3\n",
            "INFO: include_order.sv:6:5: [SIM-001] (tb @ 0 ps) $finish called\n",
            &[],
            &["--edition", edition, "--include-dir", later],
        );
    }
}

#[test]
fn escaped_identifier_with_macro_range_executes() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "escaped_range",
            "escaped=1000001\n",
            "INFO: escaped_range.sv:9:5: [SIM-001] (tb @ 0 ps) $finish called\n",
            &[],
            &["--edition", edition],
        );
    }
}

#[test]
fn standard_attribute_is_simulation_neutral() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::run_case_with_args(
            SUITE,
            "attribute_neutral",
            "attribute=a\n",
            "INFO: attribute_neutral.sv:8:5: [SIM-001] (tb @ 0 ps) $finish called\n",
            &[],
            &["--edition", edition],
        );
    }
}
