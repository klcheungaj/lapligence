//! P07 acceptance through the public HDL frontend and both optimizer modes.
//! Migration rejections are failures here, not expected or silently skipped cases.

#[path = "support/sim_cli.rs"]
mod sim_cli;
#[path = "support/sim.rs"]
mod sim_harness;

fn run(fixture: &str, expected: &str) {
    sim_cli::run_case("dynamic_ownership", fixture, expected, "", &[]);
}

#[test]
fn numeric_loop() {
    run("numeric_loop", "1007\n");
}

#[test]
fn branch_side_effects() {
    run("branch_side_effects", "23 0\n1\n");
}

#[test]
fn wider_intermediate_than_declared_storage() {
    run("wide_intermediate", "7168 4096\n");
}

#[test]
fn mixed_width_loop() {
    run("mixed_width_loop", "8192 1\n");
}

#[test]
fn selected_nba_captures_before_mutation() {
    run("selected_nba_capture", "0\n42\n");
}

#[test]
fn recursive_return() {
    run("recursive_return", "2176\n");
}

#[test]
fn task_values_survive_yield_and_copyout() {
    run("yielding_task", "47\n");
}

#[test]
fn finish_cancels_live_process() {
    run("finish_cleanup", "finish\n");
}

#[test]
fn numeric_function_argument_never_requests_a_c_fragment() {
    run("function_numeric_input", "42\n");
}

#[test]
fn default_argument_references_earlier_typed_formal() {
    run("numeric_default_argument", "42\n");
}

#[test]
fn numeric_inout_copyin_survives_a_yield() {
    run("numeric_inout_argument", "42 43\n");
}

#[test]
fn event_array_indices_release_their_owners() {
    run("event_array_owners", "event\n");
}

#[test]
fn evaluated_wait_and_qualifier_have_owned_results() {
    run("evaluated_event_owners", "qualified\n");
}

#[test]
fn detached_numeric_captures_survive_parent_iteration() {
    run("captured_fork_owners", "0\n1\n2\n");
}

#[test]
fn nba_issue_capture_freezes_value_and_destination() {
    sim_cli::run_case(
        "feature_completion/g1_19",
        "nba_issue_capture",
        "x=05 mem0=9 mem1=b\n",
        "",
        &[],
    );
}

#[test]
fn nba_ordered_updates_commit_in_issue_order() {
    sim_cli::run_case(
        "feature_completion/g1_19",
        "nba_ordered_updates",
        "x=d5 y=03\n",
        "",
        &[],
    );
}

#[test]
fn nba_illegal_lifetime_stays_rejected() {
    sim_cli::reject_case(
        "feature_completion/g1_19",
        "nba_automatic_local",
        "nonblocking assignment to automatic variable",
    );
    for optimized in [false, true] {
        let output = sim_cli::invoke_with_env(
            "feature_completion/g1_19",
            "nba_ref_formal",
            optimized,
            &[],
            &[],
            &[],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "NBA through a ref was accepted");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(
            stderr.contains("nonblocking assignment to automatic")
                || stderr.contains("targets stack-backed input/formal/local storage")
                || stderr.contains("nonblocking writes through reference formals")
                || stderr.contains("nonblocking assignment requires persistent target storage"),
            "{stderr}",
        );
    }
}

#[test]
fn owner_publication_snapshot() {
    // The wide block-local owner is overwritten/released before the NBA
    // commits; the committed snapshot and the reset source are both checked.
    sim_cli::run_case(
        "feature_completion/g1_05",
        "owner_publication_snapshot",
        "7\n0\n",
        "",
        &[],
    );
}

#[test]
fn owner_cancel_unwind() {
    // A disabled suspended branch and a second live detached owner unwound by
    // $finish must not leak, double free, or commit their wide temporaries.
    sim_cli::run_case(
        "feature_completion/g1_05",
        "owner_cancel_unwind",
        "shared=1\n",
        "",
        &[],
    );
}

#[test]
fn owner_allocation_plateau() {
    // Repeated equal-size wide owners must not accumulate: the loop's exact
    // result is the behavioral oracle, while the native tracked-allocation
    // benchmark checks the live plateau.
    sim_cli::run_case(
        "feature_completion/g1_05",
        "owner_allocation_plateau",
        "140000\n",
        "",
        &[],
    );
}
