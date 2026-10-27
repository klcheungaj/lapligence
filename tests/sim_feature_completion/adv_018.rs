//! ADV-018: DPI-C call chains whose exported tasks consume time (IEEE
//! 1800-2009 35.5.1.5, 35.8, 35.9). Every fixture has a checked-in C
//! companion, built here into a shared library (a DLL linked against the
//! model's import library on Windows) or a static archive and passed with
//! `--dpi-lib`; the expected outputs are derived by hand in the fixture
//! readme. The foreign code of each suspendable import runs on a foreign
//! call context (`src/sim/rt/foreign_contexts/readme.md`).
use super::{sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/adv_018";

fn companion(directory: &sim_harness::TempDir, stem: &str) -> Option<String> {
    super::sim_040::shared_companion(SUITE, directory, stem, &[])
}

/// A01: chains suspended at once resume in crossing and in nested order,
/// each with intact C locals.
#[test]
fn suspended_chains_resume_in_any_order() {
    let directory = sim_harness::TempDir::new("adv018").expect("temporary directory");
    let Some(library) = companion(&directory, "crossing") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "crossing",
        include_str!("../fixtures/sim/feature_completion/adv_018/crossing.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

/// The static-archive route (no import library, every host) gives the same
/// output.
#[test]
fn static_archive_companion_suspends_too() {
    let directory = sim_harness::TempDir::new("adv018-static").expect("temporary directory");
    let archive = super::sim_041::static_archive(SUITE, &directory, "crossing", &[]);
    sim_cli::run_case_backend_parity(
        SUITE,
        "crossing",
        include_str!("../fixtures/sim/feature_completion/adv_018/crossing.out"),
        &["--dpi-lib", &archive],
        &[],
    );
}

/// A02: nested C -> SV -> C -> SV chains with time and event waits, a fork
/// in the export, svSetScope moves and their restoration, and export
/// inputs and outputs across suspensions.
#[test]
fn nested_chains_keep_scopes_locals_and_arguments() {
    let directory = sim_harness::TempDir::new("adv018").expect("temporary directory");
    let Some(library) = companion(&directory, "nested") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "nested",
        include_str!("../fixtures/sim/feature_completion/adv_018/nested.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

/// A02: disables of suspended chains (35.9), a kill that unwinds a chain,
/// and `$finish` with a chain still suspended, which ends the run cleanly.
#[test]
fn suspended_chains_follow_the_disable_protocol() {
    let directory = sim_harness::TempDir::new("adv018").expect("temporary directory");
    let Some(library) = companion(&directory, "disables") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "disables",
        include_str!("../fixtures/sim/feature_completion/adv_018/disables.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

/// A fatal error after the output that precedes it.
fn fatal_after(
    stdout: &'static str,
    message: &'static str,
) -> impl Fn(&str, &std::process::Output) {
    move |label, output| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{label}: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), stdout, "{label}");
        assert!(stderr.contains(message), "{label}: {stderr}");
        assert!(
            stderr.contains("DPI-C protocol violation"),
            "{label}: {stderr}"
        );
    }
}

/// A03 and 35.9: an imported function that calls an exported task that can
/// consume time, a missing acknowledgement of a disable and an export call
/// from an unwinding chain are fatal errors; none of them hangs.
#[test]
fn suspension_protocol_violations_are_fatal_errors() {
    let directory = sim_harness::TempDir::new("adv018-errors").expect("temporary directory");
    // Every design declares the export errors.c calls, so on Windows the
    // import library of one of them serves the whole DLL.
    let Some(built) =
        super::sim_040::try_shared_companion(SUITE, &directory, "errors", "neg_function_wait", &[])
    else {
        return;
    };
    let library = built
        .unwrap_or_else(|error| panic!("errors.c: {error}"))
        .link
        .to_string_lossy()
        .into_owned();
    let args = ["--dpi-lib", library.as_str()];
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_function_wait",
        &args,
        &fatal_after(
            "start\nc_func enter\nc_func status=0\n",
            "llg: DPI-C exported task `sv_wait_e` can consume time, but its foreign caller cannot suspend: only a context imported task may call it (SV 35.8)",
        ),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_missing_ack",
        &args,
        &fatal_after(
            "start\nc_no_ack enter\nc_no_ack status=1 returns 0\n",
            "llg: DPI-C import task `c_no_ack` returned 0 although a disable cancelled it (SV 35.9 item b)",
        ),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_kill_call_again",
        &args,
        &fatal_after(
            "start\nc_again enter\nc_again status=1 disabled=1\n",
            "llg: DPI-C export `sv_wait_e` was called after a disable cancelled the calling import (SV 35.9 item d)",
        ),
    );
}

/// Only context imported tasks of a design with an exported task that can
/// consume time become suspendable shims; the generated model of a design
/// with zero-time exports only keeps the direct call path and links no
/// thread library.
#[test]
fn only_designs_with_waiting_exports_get_context_shims() {
    let directory = sim_harness::TempDir::new("adv018-gen").expect("temporary directory");
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let generate = |design: &str, out: &str| {
        let output = sim_harness::run_command(
            std::process::Command::new(env!("CARGO_BIN_EXE_llg"))
                .current_dir(directory.path())
                .args(["--top", "tb", "--gen-only", "--out-dir", out])
                .arg(root.join(format!("tests/fixtures/sim/{design}.sv"))),
            std::time::Duration::from_secs(120),
        )
        .expect("llg should start");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let model = directory.path().join(out).join("sim/tb/model.c");
        let cmake = directory.path().join(out).join("sim/tb/CMakeLists.txt");
        (
            std::fs::read_to_string(model).expect("model.c"),
            std::fs::read_to_string(cmake).expect("CMakeLists.txt"),
        )
    };
    let (model, cmake) = generate("feature_completion/adv_018/crossing", "waiting");
    assert!(model.contains("llg_dpi_context_begin("), "{model}");
    assert!(model.contains("llg_dpi_export_begin("), "{model}");
    assert!(cmake.contains("Threads::Threads"), "{cmake}");
    let (model, cmake) = generate("feature_completion/sim_041/chains", "zero_time");
    assert!(!model.contains("llg_dpi_context"), "{model}");
    assert!(!model.contains("llg_dpi_export_begin("), "{model}");
    assert!(!cmake.contains("Threads::Threads"), "{cmake}");
}
