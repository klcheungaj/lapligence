//! SIM-004: persistent string/chandle nonblocking writes, native value
//! links and native conditional values. Oracles are derived by hand in the
//! fixture readme.
use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/sim_004";

#[test]
fn persistent_string_and_record_nbas_commit_issue_time_values() {
    let expected = include_str!("../fixtures/sim/feature_completion/sim_004/native_nba.out");
    sim_cli::run_case_backend_parity(SUITE, "native_nba", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_nba", expected);
}

#[test]
fn chandle_nbas_keep_borrowed_foreign_pointers() {
    if !cfg!(unix) {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return;
    }
    let directory = sim_harness::TempDir::new("sim004-dpi").expect("temporary directory");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/sim_004/native_chandle_nba.c");
    let library = directory.path().join("libsim004_foreign.so");
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    let output = sim_harness::run_command(
        std::process::Command::new(compiler)
            .args(["-shared", "-fPIC"])
            .arg(&source)
            .arg("-o")
            .arg(&library),
        std::time::Duration::from_secs(60),
    )
    .expect("C compiler for the DPI fixture");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let library = library.to_string_lossy();
    sim_cli::run_case_backend_parity(
        SUITE,
        "native_chandle_nba",
        include_str!("../fixtures/sim/feature_completion/sim_004/native_chandle_nba.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

#[test]
fn adopted_native_nba_witnesses() {
    sim_cli::run_case(
        SUITE,
        "witness_string_nba",
        include_str!("../fixtures/sim/feature_completion/sim_004/witness_string_nba.out"),
        "",
        &[],
    );
    sim_cli::run_case(
        SUITE,
        "witness_chandle_nba",
        include_str!("../fixtures/sim/feature_completion/sim_004/witness_chandle_nba.out"),
        "",
        &[],
    );
    sim_cli::run_case(
        SUITE,
        "witness_string_record_nba",
        include_str!("../fixtures/sim/feature_completion/sim_004/witness_string_record_nba.out"),
        "",
        &[],
    );
}

#[test]
fn illegal_native_nba_targets_stay_rejected() {
    sim_cli::reject_case(
        SUITE,
        "neg_string_element_nba",
        "nonblocking assignment to a string element in `tb` is illegal",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_class_string_nba",
        "nonblocking assignment to a class property in `tb` is illegal",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_string_nba",
        "nonblocking assignment to automatic variable 's' is not allowed",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_automatic_monitor",
        "automatic variable 's' cannot be traced",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_chandle_continuous",
        "cannot assign to a chandle outside of a procedural context",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_chandle_port",
        "is not a valid type for a port because it contains type 'chandle'",
    );
}

#[test]
fn unsupported_native_nba_boundaries_are_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_static_native_record_nba",
        "nonblocking assignment to native record subroutine storage in `tb` is not supported",
    );
}
