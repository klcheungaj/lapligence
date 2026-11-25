//! Updates queued by the operands of a marked nonblocking assignment
//! ([readme](../fixtures/sim/feature_completion/ki_located_updates/readme.md)).

use super::{c_compiler, sim_cli, sim_harness};

const SUITE: &str = "feature_completion/ki_located_updates";

#[test]
fn operands_queue_their_own_updates() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "operands",
        include_str!("../fixtures/sim/feature_completion/ki_located_updates/operands.out"),
        &[],
        &[],
    );
}

#[test]
fn operands_of_held_updates_queue_theirs_at_issue() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "holds",
        include_str!("../fixtures/sim/feature_completion/ki_located_updates/holds.out"),
        &[],
        &[],
    );
}

#[test]
fn operands_that_end_the_process_leave_no_mark() {
    let directory = sim_harness::TempDir::new("ki-located-updates").expect("temporary directory");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("tests/fixtures/sim/{SUITE}/ends.c"));
    let library = c_compiler::static_archive(&source, directory.path(), "ends", &[])
        .expect("DPI-C library")
        .to_string_lossy()
        .into_owned();
    sim_cli::run_case_backend_parity(
        SUITE,
        "ends",
        include_str!("../fixtures/sim/feature_completion/ki_located_updates/ends.out"),
        &["--dpi-lib", &library],
        &[],
    );
}
