//! SYN-035 exclusion checkpoint: a selected PLA target has not been defined.

use crate::sim_cli;

#[test]
fn unselected_pla_task_reports_an_explicit_diagnostic_in_both_editions() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::reject_case_with_args(
            "syn035_pla",
            "pla_unselected",
            "unsupported PLA system task `$async$and$array`",
            &["--edition", edition],
        );
    }
}
