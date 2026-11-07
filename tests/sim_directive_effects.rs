//! SYN-017: preprocessing and directive state must survive into simulation.

use crate::sim_cli;

const SUITE: &str = "directive_effects";

fn editions() -> [&'static str; 2] {
    ["v2001", "sv2009"]
}

#[test]
fn macro_include_and_conditional_state_reaches_execution() {
    for (defines, branch) in [
        (&["--define", "ENABLE"][..], 11),
        (&["--define", "ALT"][..], 22),
        (&[][..], 33),
    ] {
        let args: Vec<&str> = if defines.is_empty() {
            vec!["--edition", "sv2009"]
        } else {
            vec!["--edition", "sv2009", defines[0], defines[1]]
        };
        let expected = format!("branch={branch} cat=a text=syn017\n");
        sim_cli::run_case_with_args(
            SUITE,
            "macros_include",
            &expected,
            "INFO: macros_include.sv:19:5: [SIM-001] (tb @ 0 ps) $finish called\n",
            &[],
            &args,
        );
    }
}

#[test]
fn resetall_restores_default_nettype() {
    for edition in editions() {
        let args = ["--edition", edition];
        sim_cli::run_case_with_args(
            SUITE,
            "default_resetall",
            "implicit=z\n",
            "INFO: default_resetall.sv:12:5: [SIM-001] (tb @ 0 ps) $finish called\n",
            &[],
            &args,
        );
    }
}

#[test]
fn default_nettype_none_rejects_implicit_net() {
    for edition in editions() {
        let args = ["--edition", edition];
        sim_cli::reject_case_with_args(
            SUITE,
            "default_nettype_none",
            "use of undeclared identifier",
            &args,
        );
    }
}

#[test]
fn line_directive_reaches_predefined_macros() {
    sim_cli::run_case_with_args(
        SUITE,
        "line_mapping",
        "file=syn017_mapped.sv line=125\n",
        "INFO: line_mapping.sv:9:5 (`line syn017_mapped.sv:126): [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
        &["--edition", "sv2009"],
    );
}

#[test]
fn line_directive_keeps_owned_physical_diagnostic_range() {
    for edition in editions() {
        let args = ["--edition", edition];
        sim_cli::reject_case_with_args(
            SUITE,
            "line_mapping_error",
            "line_mapping_error.sv:7:10",
            &args,
        );
    }
}

#[test]
fn unconnected_drive_values_are_lowered_after_elaboration() {
    for edition in editions() {
        let args = ["--edition", edition];
        sim_cli::run_case_with_args(
            SUITE,
            "unconnected_matrix",
            "p0=0 p1=f pz=z\n",
            "INFO: unconnected_matrix.sv:24:5: [SIM-001] (tb @ 0 ps) $finish called\n",
            &[],
            &args,
        );
    }
}

#[test]
fn vectored_and_scalared_values_are_simulation_neutral() {
    for edition in editions() {
        let args = ["--edition", edition];
        sim_cli::run_case_with_args(
            SUITE,
            "vectored_scalared",
            "v=a s=5\n",
            "INFO: vectored_scalared.sv:12:5: [SIM-001] (tb @ 0 ps) $finish called\n",
            &[],
            &args,
        );
    }
}

#[test]
fn macro_generated_systemverilog_keyword_observes_edition() {
    sim_cli::run_case_with_args(
        SUITE,
        "later_macro",
        "x=1\n",
        "INFO: later_macro.sv:13:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &["combinational always process in `tb` reads no signals; evaluating once at time 0"],
        &["--edition", "sv2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "later_macro",
        "expected a declaration name",
        &["--edition", "v2001"],
    );
}

#[test]
fn unavailable_include_is_rejected_in_both_editions() {
    for edition in editions() {
        let args = ["--edition", edition];
        sim_cli::reject_case_with_args(
            SUITE,
            "missing_include",
            // The reason after the name is the host C++ library's text
            // ("No such file..." on glibc, "no such file..." on MSVC).
            "'missing_syn017.svh': ",
            &args,
        );
    }
}

#[test]
fn compilation_unit_mode_preserves_source_order() {
    for edition in editions() {
        for (mode, expected) in [("separate", "unit_flag=0\n"), ("merged", "unit_flag=1\n")] {
            let args = ["--edition", edition, "--compilation-units", mode];
            sim_cli::run_case_with_source_prefix(
                SUITE,
                "unit_use",
                &["unit_def"],
                expected,
                "INFO: unit_use.sv:11:5: [SIM-001] (tb @ 0 ps) $finish called\n",
                &[],
                &args,
            );
        }
    }
}
