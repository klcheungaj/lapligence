//! SIM-028: object, process and shuffle random streams and the legacy
//! distribution functions. Random-stream oracles are relations derived from
//! IEEE 1800-2009 18.13-18.15 in the fixture readme; legacy values come from
//! the independent Annex N transcription `annex_n_reference.c`.
use std::cell::RefCell;
use std::time::Duration;

use super::{c_compiler, sim_cli, sim_harness};

const SUITE: &str = "feature_completion/sim_028";
const ANNEX_N: &str = include_str!("../fixtures/sim/feature_completion/sim_028/annex_n.out");

#[test]
fn legacy_distributions_match_the_annex_n_reference() {
    sim_cli::run_case_backend_parity(SUITE, "annex_n", ANNEX_N, &[], &[]);
}

/// The checked-in vectors are the output of the standalone Annex N
/// transcription, rebuilt here with the host C compiler at both optimization
/// levels.
#[test]
fn annex_n_reference_program_reproduces_the_vectors() {
    let compiler = c_compiler::host_c_compiler();
    if !c_compiler::c_compiler_available(&compiler) {
        eprintln!("SKIP: C compiler `{compiler}` not available");
        return;
    }
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/feature_completion/sim_028/annex_n_reference.c");
    for (label, level) in [
        ("O0", c_compiler::Optimize::None),
        ("O2", c_compiler::Optimize::Speed),
    ] {
        let dir = sim_harness::TempDir::new(&format!("annex-n-{label}"))
            .expect("create reference directory");
        std::fs::copy(&source, dir.path().join("annex_n_reference.c"))
            .expect("copy reference program");
        let (mut command, executable) = c_compiler::strict_c11_executable(
            &compiler,
            dir.path(),
            level,
            &["annex_n_reference.c"],
            "annex_n_reference",
        );
        let output = sim_harness::run_command(&mut command, Duration::from_secs(60))
            .unwrap_or_else(|error| panic!("compile Annex N reference at {label}: {error}"));
        assert!(
            output.status.success(),
            "Annex N reference must compile at {label}:\n{}",
            c_compiler::compiler_diagnostics(&output)
        );
        let stdout = sim_harness::run_executable(&executable).expect("run Annex N reference");
        assert_eq!(stdout, ANNEX_N, "{label}");
    }
}

#[test]
fn writable_seeds_through_formals_elements_and_widths() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "seed_aliasing",
        include_str!("../fixtures/sim/feature_completion/sim_028/seed_aliasing.out"),
        &[],
        &[],
    );
}

#[test]
fn object_streams_seed_save_and_restore() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "object_streams",
        include_str!("../fixtures/sim/feature_completion/sim_028/object_streams.out"),
        &[],
        &[],
    );
}

#[test]
fn thread_state_replays_the_next_values() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "state_replay",
        include_str!("../fixtures/sim/feature_completion/sim_028/state_replay.out"),
        &[],
        &[],
    );
}

#[test]
fn shuffle_is_a_permutation_drawn_from_the_calling_thread() {
    sim_cli::run_case_backend_parity(
        SUITE,
        "shuffle_streams",
        include_str!("../fixtures/sim/feature_completion/sim_028/shuffle_streams.out"),
        &[],
        &[],
    );
}

/// The values are implementation dependent, so the oracle is a relation: the
/// same design yields the same values on every backend and optimizer mode, and
/// unrelated activity (`EXTRA_ACTIVITY`) changes only its own `extra` lines.
#[test]
fn unrelated_activity_does_not_perturb_streams() {
    let outputs = RefCell::new(Vec::new());
    for args in [&[][..], &["-D", "EXTRA_ACTIVITY"][..]] {
        sim_cli::run_case_checked_matrix(SUITE, "thread_stability", args, &|label, output| {
            assert!(output.status.success(), "{label}, {args:?}: {output:?}");
            let stdout = String::from_utf8_lossy(&output.stdout);
            let kept = stdout
                .lines()
                .filter(|line| !line.starts_with("extra"))
                .map(|line| format!("{line}\n"))
                .collect::<String>();
            outputs
                .borrow_mut()
                .push((format!("{label}, {args:?}"), kept));
        });
    }
    let outputs = outputs.into_inner();
    let (first_label, first) = &outputs[0];
    assert_eq!(first.lines().count(), 7, "{first_label}: {first}");
    assert!(
        first.contains("differ 1\n"),
        "{first_label}: identical instances must draw different values: {first}"
    );
    for (label, output) in &outputs[1..] {
        assert_eq!(output, first, "{label} differs from {first_label}");
    }
}

#[test]
fn class_cannot_declare_a_builtin_random_method() {
    sim_cli::reject_case(
        SUITE,
        "neg_srandom_override",
        "cannot override built-in method 'srandom'",
    );
}

#[test]
fn foreign_random_state_is_a_runtime_error() {
    sim_cli::reject_case(
        SUITE,
        "neg_randstate_invalid",
        "llg: random runtime: invalid randstate string",
    );
}

#[test]
fn random_method_through_a_null_handle_is_a_runtime_error() {
    sim_cli::reject_case(
        SUITE,
        "neg_srandom_null",
        "null class handle access: method call at tb:10:5",
    );
}
