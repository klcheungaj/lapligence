//! `foreach` loop variables captured by a detached fork
//! ([readme](../fixtures/sim/feature_completion/ki_foreach_fork_capture/readme.md)).

use super::sim_cli;
use super::sim_harness;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const SUITE: &str = "feature_completion/ki_foreach_fork_capture";

#[test]
fn detached_fork_branches_share_the_foreach_loop_variable() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "forks",
        include_str!("../fixtures/sim/feature_completion/ki_foreach_fork_capture/forks.out"),
        &[],
        &[],
    );
}

#[test]
fn loops_inside_fork_branches_keep_their_own_variables() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "branch_loops",
        include_str!("../fixtures/sim/feature_completion/ki_foreach_fork_capture/branch_loops.out"),
        &[],
        &[],
    );
    sim_cli::run_case_backend_parity(
        SUITE,
        "branch_loop_forks",
        include_str!(
            "../fixtures/sim/feature_completion/ki_foreach_fork_capture/branch_loop_forks.out"
        ),
        &[],
        &[],
    );
}

/// The heap frame allocations in the generated model of `fixture`.
fn frame_allocations(fixture: &str, optimized: bool) -> usize {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim")
        .join(SUITE)
        .join(format!("{fixture}.sv"));
    let directory = sim_harness::TempDir::new("foreach-fork-shape").expect("scratch");
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command
        .args(["--gen-only", "--top", "tb", "--out-dir"])
        .arg(directory.path());
    if !optimized {
        command.arg("--no-opt");
    }
    let output = sim_harness::run_command(command.arg(&source), Duration::from_secs(60))
        .expect("generate the foreach model");
    assert!(output.status.success(), "{fixture}: {output:?}");
    std::fs::read_to_string(directory.path().join("sim/tb/model.c"))
        .expect("model source")
        .matches("llg_frame_new(")
        .count()
}

#[test]
fn loops_inside_fork_branches_keep_local_iterators() {
    for optimized in [true, false] {
        // A loop that sits inside a branch, with no fork in its body that
        // names its variable, belongs to that branch's process alone: its
        // iterator stays a local, so `branch_loops` needs no frame at all.
        assert_eq!(
            frame_allocations("branch_loops", optimized),
            0,
            "branch_loops, optimized={optimized}"
        );
        // The same position with a detached fork in the loop body that names
        // the variable shares it through a frame cell.
        assert!(
            frame_allocations("branch_loop_forks", optimized) > 0,
            "branch_loop_forks, optimized={optimized}"
        );
    }
}
