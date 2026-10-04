//! SIM-003: descriptor-backed native record values, explicit roots and their
//! legal boundaries. Oracles are derived by hand in the fixture readme.
use super::{sim_cli, sim_harness};

const SUITE: &str = "feature_completion/sim_003";

#[test]
fn native_records_cross_calls_by_value_on_every_backend() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_003/native_record_calls.out");
    sim_cli::run_case_backend_parity(SUITE, "native_record_calls", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_record_calls", expected);
}

#[test]
fn native_record_roots_survive_recursion_cancellation_and_churn() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_003/native_record_lifetimes.out");
    sim_cli::run_case_backend_parity(SUITE, "native_record_lifetimes", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_record_lifetimes", expected);
}

#[test]
fn native_record_reads_drive_continuous_and_comb_logic() {
    let expected =
        include_str!("../fixtures/sim/feature_completion/sim_003/native_record_sensitivity.out");
    sim_cli::run_case_backend_parity(SUITE, "native_record_sensitivity", expected, &[], &[]);
    sim_cli::run_case_after_db_drop(SUITE, "native_record_sensitivity", expected);
}

#[test]
fn adopted_native_record_witnesses() {
    sim_cli::run_case(
        SUITE,
        "native_record_call",
        include_str!("../fixtures/sim/feature_completion/sim_003/native_record_call.out"),
        "",
        &[],
    );
    sim_cli::run_case(
        SUITE,
        "native_equality",
        include_str!("../fixtures/sim/feature_completion/sim_003/native_equality.out"),
        "",
        &[],
    );
    sim_cli::run_case(
        SUITE,
        "chandle_record_call",
        include_str!("../fixtures/sim/feature_completion/sim_003/chandle_record_call.out"),
        "",
        &[],
    );
}

#[test]
fn borrowed_chandles_in_native_records_are_never_freed() {
    if !cfg!(unix) {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return;
    }
    let directory = sim_harness::TempDir::new("sim003-dpi").expect("temporary directory");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/sim_003/native_borrowed_chandle.c");
    let library = directory.path().join("libsim003_foreign.so");
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
    sim_cli::run_case_with_args(
        SUITE,
        "native_borrowed_chandle",
        include_str!("../fixtures/sim/feature_completion/sim_003/native_borrowed_chandle.out"),
        "",
        &[],
        &["--dpi-lib", &library],
    );
}

#[test]
fn chandles_are_never_packed_or_arithmetic() {
    sim_cli::reject_case(
        SUITE,
        "neg_chandle_packed",
        "packed members must be of integral type (not 'chandle')",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_chandle_arithmetic",
        "invalid operands to binary expression ('chandle' and 'int')",
    );
}

#[test]
fn unsupported_native_record_boundaries_are_explicit() {
    sim_cli::reject_case(
        SUITE,
        "neg_native_ref_formal",
        "ref formal `x` of native record type is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_native_fork_capture",
        "capturing native subroutine storage in a fork is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_native_runtime_index",
        "runtime index into a native record array member is not supported",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_native_conditional",
        "native record conditional in `tb` is not supported",
    );
}
