//! Run-time errors, `$finish` and `$fatal` inside static declaration
//! initializers (IEEE 1800-2009 6.8): each behaves as the same event in a
//! process. One that ends the run stops the initializers at once, and no
//! procedure starts; any other reports and the run goes on. Clauses and
//! oracles are in the fixture readme.
use super::sim_cli;

const SUITE: &str = "feature_completion/ki_static_init_errors";

#[test]
fn coverage_option_error_reports_and_the_run_goes_on() {
    sim_cli::run_case_stopping_at_runtime(
        SUITE,
        "coverage_option",
        &[],
        include_str!(
            "../fixtures/sim/feature_completion/ki_static_init_errors/coverage_option.out"
        ),
        "SIM-010",
        "coverage option.weight value -1 is out of range; the option keeps its value",
    );
}

#[test]
fn index_check_stop_ends_the_run_at_the_access() {
    sim_cli::run_case_stopping_at_runtime(
        SUITE,
        "index_check",
        &[],
        include_str!("../fixtures/sim/feature_completion/ki_static_init_errors/index_check.out"),
        "SIM-009",
        "queue insert: index 5 is outside the valid range 0 to 1",
    );
}

#[test]
fn null_handle_access_ends_the_run_at_the_access() {
    sim_cli::run_case_stopping_at_runtime(
        SUITE,
        "null_access",
        &[],
        include_str!("../fixtures/sim/feature_completion/ki_static_init_errors/null_access.out"),
        "SIM-009",
        "null class handle access",
    );
}

#[test]
fn error_task_reports_and_initialization_goes_on() {
    sim_cli::run_case(
        SUITE,
        "error_task",
        include_str!("../fixtures/sim/feature_completion/ki_static_init_errors/error_task.out"),
        "ERROR: error_task.sv:8:5: [SIM-005] (counter @ 0 ns) class static initializer error\n\
         ERROR: error_task.sv:8:5: [SIM-005] (counter @ 0 ns) module initializer error\n\
         INFO: error_task.sv:23:5: [SIM-001] (tb @ 1 ns) $finish called\n\
         INFO: [SIM-000] 2 error(s), 0 warning(s), 1 info(s)\n",
        &[],
    );
}

#[test]
fn fatal_task_ends_the_run_at_the_call() {
    sim_cli::run_case(
        SUITE,
        "fatal_task",
        include_str!("../fixtures/sim/feature_completion/ki_static_init_errors/fatal_task.out"),
        "FATAL: fatal_task.sv:8:5: [SIM-006] (p @ 0 ps) package initializer fatal\n\
         INFO: fatal_task.sv:8:5: [SIM-001] (p @ 0 ps) $finish called\n\
         INFO: [SIM-000] 1 fatal, 0 error(s), 0 warning(s), 1 info(s)\n",
        &[],
    );
}

#[test]
fn finish_task_ends_the_run_at_the_call() {
    sim_cli::run_case(
        SUITE,
        "finish_task",
        include_str!("../fixtures/sim/feature_completion/ki_static_init_errors/finish_task.out"),
        "INFO: finish_task.sv:10:5: [SIM-001] (tb @ 0 ps) $finish called\n",
        &[],
    );
}
