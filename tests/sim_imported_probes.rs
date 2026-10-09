//! Focused acceptance witnesses supplied separately from the existing feature suites.

use crate::sim_cli;

const SUITE: &str = "imported_probes/acceptance";

#[test]
fn femtosecond_delay_uses_local_picosecond_units() {
    sim_cli::run_case(
        SUITE,
        "Femtosecond_Delay",
        "CHECK: elapsed=0.001\n",
        "",
        &[],
    );
}

/// Procedural assign/deassign is unsupported by design (ADV-001, user
/// decision 2026-10-09): both supplied probes stop at every statement.
#[test]
fn procedural_assign_probes_are_rejected_as_unsupported() {
    let family = "(legacy procedural assign/deassign form) is not supported by llg";
    for (fixture, sites) in [
        (
            "Procedural_Assign_Replacement.sv",
            &[(6, "assign"), (8, "assign"), (12, "deassign")][..],
        ),
        (
            "Procedural_Assign_Priority.sv",
            &[(6, "assign"), (11, "deassign")][..],
        ),
    ] {
        let lines: Vec<String> = sites
            .iter()
            .map(|(line, stmt)| format!("{line}:5: unsupported: procedural `{stmt}` {family}"))
            .collect();
        sim_cli::reject_case_with_error_lines(SUITE, fixture, &lines, &[]);
    }
}

#[test]
fn nonblocking_event_wakes_after_active_region_write() {
    sim_cli::run_case(
        SUITE,
        "Nonblocking_Event_Triggers_In_NBA",
        "CHECK: stage=1\n",
        "",
        &[],
    );
}

#[test]
fn reference_argument_writes_through_to_caller() {
    sim_cli::run_case(
        SUITE,
        "Reference_Argument_Is_An_Alias",
        "CHECK: value=9\n",
        "",
        &[],
    );
}

#[test]
fn time_query_rounds_each_local_unit() {
    sim_cli::run_case(
        SUITE,
        "Time_Query_Rounds_Local_Units",
        "CHECK: first=2\nCHECK: second=3\n",
        "",
        &[],
    );
}

#[test]
fn net_alias_connectivity() {
    sim_cli::run_case(SUITE, "Net_Alias_Connectivity", "CHECK: b=1\n", "", &[]);
}

#[test]
fn alias_delayed_wakeup_notifies_waiters() {
    // `wire #2 original` plus `alias original = mirror`: the delayed network
    // publication must still wake an `@`/wait observer of the alias name.
    sim_cli::run_case(
        "imported_probes/counterexamples",
        "Alias_Delayed_Wakeup",
        "",
        "",
        &[],
    );
}

#[test]
fn alias_postponed_read_is_pure() {
    // A `$strobe` reader of an alias in the Postponed region must observe the
    // published value without refreshing storage as a side effect of the read.
    sim_cli::run_case(
        "imported_probes/counterexamples",
        "Alias_Postponed_Read",
        "alias=1\n",
        "",
        &[],
    );
}
