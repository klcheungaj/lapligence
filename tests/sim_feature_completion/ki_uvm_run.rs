//! Forms a verification library's run reaches once its classes generate:
//! automatic string, class handle, chandle and process locals a subroutine
//! never names again. Clauses and oracles are in the fixture readme; the
//! generated-C sweep (`generated_c_frame_lint`) compiles the same fixtures
//! with `-Wall` and accepts no warning.
use super::sim_cli;

const SUITE: &str = "feature_completion/ki_uvm_run";

#[test]
fn unused_locals() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "unused_locals",
        include_str!("../fixtures/sim/feature_completion/ki_uvm_run/unused_locals.out"),
        &[],
        &[],
    );
}
