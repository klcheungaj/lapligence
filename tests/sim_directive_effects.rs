//! SYN-017: preprocessing and directive state must survive into simulation.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

const SUITE: &str = "directive_effects";

fn editions() -> [&'static str; 2] {
    ["2001", "2009"]
}

#[test]
fn macro_include_and_conditional_state_reaches_execution() {
    for edition in editions() {
        for (defines, branch) in [
            (&["--define", "ENABLE"][..], 11),
            (&["--define", "ALT"][..], 22),
            (&[][..], 33),
        ] {
            let args: Vec<&str> = if defines.is_empty() {
                vec!["--edition", edition]
            } else {
                vec!["--edition", edition, defines[0], defines[1]]
            };
            let expected = format!("branch={branch} cat=a text=syn017\n");
            sim_cli::run_case_with_args(
                SUITE,
                "macros_include",
                &expected,
                "llg: $finish at time 0 at tb:20:5\n",
                &[],
                &args,
            );
        }
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
            "llg: $finish at time 0 at tb:12:5\n",
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
    for edition in editions() {
        let args = ["--edition", edition];
        sim_cli::run_case_with_args(
            SUITE,
            "line_mapping",
            "file=syn017_mapped.sv line=125\n",
            "llg: $finish at time 0 at tb:8:5\n",
            &[],
            &args,
        );
    }
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
            "llg: $finish at time 0 at tb:24:5\n",
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
            "llg: $finish at time 0 at tb:12:5\n",
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
        "llg: $finish at time 0 at tb:13:5\n",
        &["combinational always process in `tb` reads no signals; evaluating once at time 0"],
        &["--edition", "2009"],
    );
    sim_cli::reject_case_with_args(
        SUITE,
        "later_macro",
        "expected a declaration name",
        &["--edition", "2001"],
    );
}

#[test]
fn unavailable_include_is_rejected_in_both_editions() {
    for edition in editions() {
        let args = ["--edition", edition];
        sim_cli::reject_case_with_args(
            SUITE,
            "missing_include",
            "No such file or directory",
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
                "llg: $finish at time 0 at tb:11:5\n",
                &[],
                &args,
            );
        }
    }
}
