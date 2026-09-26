//! SYN-035 exclusion checkpoint: a selected PLA target has not been defined.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn unselected_pla_task_reports_an_explicit_diagnostic_in_both_editions() {
    for edition in ["2001", "2009"] {
        sim_cli::reject_case_with_args(
            "syn035_pla",
            "pla_unselected",
            "unsupported PLA system task `$async$and$array`",
            &["--edition", edition],
        );
    }
}
