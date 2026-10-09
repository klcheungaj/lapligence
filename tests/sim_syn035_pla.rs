//! SYN-035 exclusion checkpoint: PLA tasks are unsupported by design (ADV-032).

use crate::sim_cli;

#[test]
fn unselected_pla_task_reports_an_explicit_diagnostic_in_both_editions() {
    for edition in ["v2001", "sv2009"] {
        sim_cli::reject_case_with_args(
            "syn035_pla",
            "pla_unselected",
            ": unsupported: system task `$async$and$array` (legacy PLA tasks) is not supported by llg",
            &["--edition", edition],
        );
    }
}
