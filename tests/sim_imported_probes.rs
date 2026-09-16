//! Focused acceptance witnesses supplied separately from the existing feature suites.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

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

#[test]
fn replacement_assign_follows_only_new_rhs() {
    sim_cli::run_case(
        SUITE,
        "Procedural_Assign_Replacement",
        "CHECK: first=0\nCHECK: second=1\nCHECK: follows=0\n",
        "",
        &[],
    );
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
#[ignore = "net alias connectivity is not yet supported by the simulator"]
fn net_alias_connectivity() {
    sim_cli::run_case(SUITE, "Net_Alias_Connectivity", "CHECK: b=1\n", "", &[]);
}
