//! ADV-012: SDF back-annotation (`$sdf_annotate`, `--sdf`; IEEE 1800-2009
//! 32, IEEE 1364-2001 16). The SDF files and cell library of the fixtures
//! were written for these tests; every expected timestamp, value and
//! notifier is derived by hand from the clause text quoted in the fixture
//! readme. Positive cases run in both optimizer modes and on every value
//! backend.
use super::sim_cli;

const SUITE: &str = "feature_completion/adv_012";

macro_rules! fixture {
    ($name:literal) => {
        include_str!(concat!(
            "../fixtures/sim/feature_completion/adv_012/",
            $name
        ))
    };
}

/// IOPATH delays, delval pulse limits, PATHPULSE and PATHPULSEPERCENT
/// (32.7), and INCREMENT annotations applied while simulating: an event
/// already scheduled keeps its time, later events use the new delays, and
/// an incremented delay stops at 0.
#[test]
fn path_delays_and_pulse_limits() {
    sim_cli::run_case_backend_parity_with_files(
        SUITE,
        "paths.v",
        fixture!("paths.out"),
        &[],
        &[],
        &[
            ("paths.sdf", fixture!("paths.sdf")),
            ("inc.sdf", fixture!("inc.sdf")),
        ],
    );
}
