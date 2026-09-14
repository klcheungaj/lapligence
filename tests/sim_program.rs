//! End-to-end program-block coverage through the public simulator.
//!
//! The fixtures exercise Reactive/Re-Inactive/Re-NBA ordering, program
//! lifecycle completion and `$exit` cleanup.  Every positive case is run with
//! and without optimizer passes; the negative case keeps one prohibited
//! program member as its only fault.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

#[test]
fn program_reactive_nba_and_zero_delay_ordering_match_module_active() {
    sim_cli::run_case(
        "program_blocks",
        "program_basic",
        "module inactive in=0 out=x\nprogram start in=1 out=x\nprogram inactive in=1 out=x\nmodule saw program nba out=1\nmodule settled in=1 out=1\n",
        "llg: $finish at time 1000 at tb:23:9\n",
        &[],
    );
}

#[test]
fn multiple_programs_finish_naturally_and_run_finals_once() {
    sim_cli::run_case(
        "program_blocks",
        "program_natural",
        "first program\nsecond program\nmodule final\nfirst final\nsecond final\n",
        "",
        &[],
    );
}

#[test]
fn program_exit_cleans_up_children_and_runs_finals_once() {
    sim_cli::run_case(
        "program_blocks",
        "program_exit",
        "exit start\nother start\nother child start\nexit before\nother parent after\nmodule final\nexit final\nother final\n",
        "",
        &[],
    );
}

#[test]
fn program_child_exit_preserves_unrelated_program_work() {
    sim_cli::run_case(
        "program_blocks",
        "program_exit_child",
        "survivor start\nexit child start\nsurvivor after\nmodule final\nexit child final\nsurvivor final\n",
        "",
        &[],
    );
}

#[test]
fn prohibited_program_process_is_rejected() {
    sim_cli::reject_case("program_blocks", "program_prohibited", "program");
}

#[test]
fn exit_outside_program_is_ignored() {
    sim_cli::run_case(
        "program_blocks",
        "program_exit_outside",
        "module survived exit\n",
        "",
        &[],
    );
}
