//! End-of-run report: a run that ends with no future events is a normal end
//! when only permanent processes (continuous assignments, gates, links,
//! `always*`) still wait, and a deadlock when an `initial` procedure, a fork
//! branch it started or a task it called can never finish.
//!
//! The hand-written expected outputs come from IEEE 1800-2009 9.2 ("an
//! always procedure ... activity shall cease only when the simulation is
//! terminated"; "initial ... activity shall cease when the statement has
//! finished") and 9.6.1 ("Simulation automatically terminates when there is
//! no further activity of any kind"); the checked-in fixtures sit in
//! `tests/fixtures/sim/end_of_run/` (readme quotes the lines).

use crate::sim_cli;

const SUITE: &str = "end_of_run";
const NORMAL_END: &str =
    "llg: simulation ended without $finish (no processes remain) at time 1000\n";
const NORMAL_END_5: &str =
    "llg: simulation ended without $finish (no processes remain) at time 5000\n";
const DEADLOCK_0: &str =
    "llg: simulation deadlock at time 0 (waiters never woken, no future events)\n";
const DEADLOCK_1: &str =
    "llg: simulation deadlock at time 1000 (waiters never woken, no future events)\n";

macro_rules! end_case {
    ($test:ident, $fixture:literal, $stderr:expr) => {
        #[test]
        fn $test() {
            sim_cli::run_case(
                SUITE,
                $fixture,
                include_str!(concat!("fixtures/sim/end_of_run/", $fixture, ".out")),
                $stderr,
                &[],
            );
        }
    };
}

end_case!(
    continuous_assignment_waiter_is_a_normal_end,
    "assign_end",
    NORMAL_END
);
end_case!(always_waiters_are_a_normal_end, "always_end", NORMAL_END);
end_case!(
    gate_and_switch_waiters_are_a_normal_end,
    "gate_end",
    NORMAL_END
);
end_case!(
    always_comb_and_ff_waiters_are_a_normal_end,
    "comb_end",
    NORMAL_END
);
end_case!(
    fork_of_a_permanent_process_is_a_normal_end,
    "forked_end",
    NORMAL_END
);
end_case!(
    woken_initial_leaves_a_normal_end,
    "event_wakes_end",
    NORMAL_END_5
);

end_case!(
    initial_blocked_on_event_is_a_deadlock,
    "initial_event_deadlock",
    DEADLOCK_1
);
end_case!(
    initial_blocked_on_wait_is_a_deadlock,
    "initial_wait_deadlock",
    DEADLOCK_0
);
end_case!(
    blocked_fork_child_at_join_is_a_deadlock,
    "fork_join_deadlock",
    DEADLOCK_0
);
end_case!(
    blocked_join_none_child_is_a_deadlock,
    "fork_none_deadlock",
    DEADLOCK_1
);
end_case!(
    task_called_from_initial_is_a_deadlock,
    "task_deadlock",
    DEADLOCK_0
);
end_case!(
    fork_in_task_of_initial_is_a_deadlock,
    "task_fork_deadlock",
    DEADLOCK_1
);
end_case!(
    program_initial_is_a_deadlock,
    "program_deadlock",
    DEADLOCK_0
);
