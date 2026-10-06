//! Focused public-pipeline regressions for the synthesizable review findings.
use crate::sim_cli;

// Stateful static results are legal event helpers (SV 9.4.2, 13.4.2) but
// not read-only callbacks: the waiting process evaluates them once when the
// control is reached and once per dependency change. Each fixture's first
// changed result is the only detected event.
#[test]
fn stateful_static_return_mutations_are_process_evaluated_events() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r03_static_return_mutation",
        "ACCEPTED_STATEFUL_CALLBACK changes=1\n",
        "llg: $finish at time 2000 at tb:14:5\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn read_modify_write_return_access_is_process_evaluated() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r03_static_return_read_modify_write",
        "ACCEPTED_READ_MODIFY_WRITE changes=1\n",
        "llg: $finish at time 2000 at tb:16:5\n",
        &[],
        &["--edition", "2009"],
    );
}

#[test]
fn partially_assigned_static_returns_keep_their_persistent_result() {
    sim_cli::run_case_with_args(
        "review_bundle",
        "r03_static_partial_return",
        "ACCEPTED_PERSISTENT_RESULT changes=1\n",
        "llg: $finish at time 3000 at tb:15:5\n",
        &[],
        &["--edition", "2009"],
    );
}
