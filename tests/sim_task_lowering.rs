use crate::sim_cli;

#[test]
fn generated_instances_collect_timed_and_plain_subroutines() {
    sim_cli::run_case(
        "task_lowering",
        "generated_instance_timing_task",
        "counts 2 4 6 8\nstatic 2 4 6 8 plain 2 4 6 8 nested 10 10 10\nhierarchical 3 15 functions 12 15\n",
        "",
        &[],
    );
}

#[test]
fn fork_branch_waits_on_input_event_formal() {
    sim_cli::run_case(
        "task_lowering",
        "fork_event_formal",
        "hits=1 t=2\n",
        "",
        &[],
    );
}

#[test]
fn event_formals_survive_nested_and_detached_forks_and_specialization() {
    sim_cli::run_case(
        "task_lowering",
        "fork_event_variants",
        "hits=11111 triggered=11111 forwarded=11111 nested=22111 returns=1/0 specialized=2 rebound=1\n",
        "",
        &[],
    );
}
