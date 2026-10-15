//! SIM-041: zero-time DPI-C exports and context call chains. Every positive
//! fixture has a checked-in C companion, built here into a shared library
//! (Unix hosts) or a static archive (every host) and passed with `--dpi-lib`;
//! the expected outputs are derived by hand in the fixture readme from
//! IEEE 1800-2009 35.5.3, 35.7-35.9 and Annex H.9.
use super::{c_compiler, sim_cli, sim_harness};
use std::path::Path;

const SUITE: &str = "feature_completion/sim_041";

fn companion(directory: &sim_harness::TempDir, stem: &str) -> Option<String> {
    super::sim_040::shared_companion(SUITE, directory, stem, &[])
}

/// Build `<stem>.c` into a static archive that the model links directly, so
/// the companion resolves the model's exports and `svdpi.h` routines at link
/// time. This is the route on Windows, where a DLL cannot import symbols from
/// the model executable; it works on every host.
fn static_companion(directory: &sim_harness::TempDir, stem: &str) -> String {
    static_archive(SUITE, directory, stem, &[])
}

/// Build `tests/fixtures/sim/<suite>/<stem>.c` (with extra compiler `flags`)
/// into a static archive in `directory` and return its path. Shared with the
/// SIM-042 VPI fixtures, whose static route is the same `--dpi-lib` link.
pub(super) fn static_archive(
    suite: &str,
    directory: &sim_harness::TempDir,
    stem: &str,
    flags: &[&str],
) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = root.join(format!("tests/fixtures/sim/{suite}/{stem}.c"));
    let compiler = c_compiler::host_c_compiler();
    let msvc = c_compiler::is_msvc(&compiler);
    let object = directory
        .path()
        .join(format!("{stem}{}", if msvc { ".obj" } else { ".o" }));
    let mut compile = std::process::Command::new(&compiler);
    if msvc {
        compile
            .args(["/nologo", "/std:c11", "/W4", "/WX", "/c"])
            .arg(format!("/I{}", root.join("src/sim/rt").display()))
            .arg(format!("/Fo{}", object.display()));
    } else {
        compile
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-c", "-I"])
            .arg(root.join("src/sim/rt"))
            .arg("-o")
            .arg(&object);
    }
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        compile.args(flags.split_whitespace());
    }
    compile.args(flags).arg(&source);
    let output = sim_harness::run_command(&mut compile, std::time::Duration::from_secs(60))
        .expect("C compiler for the DPI fixture");
    assert!(
        output.status.success(),
        "{stem}.c: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let archive = directory.path().join(if msvc {
        format!("{stem}.lib")
    } else {
        format!("lib{stem}.a")
    });
    let mut pack = if msvc {
        let mut command = std::process::Command::new("lib");
        command
            .arg("/nologo")
            .arg(format!("/OUT:{}", archive.display()));
        command
    } else {
        let mut command = std::process::Command::new("ar");
        command.arg("rcs").arg(&archive);
        command
    };
    pack.arg(&object);
    let output = sim_harness::run_command(&mut pack, std::time::Duration::from_secs(60))
        .expect("archiver for the DPI fixture");
    assert!(
        output.status.success(),
        "{stem} archive: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    archive.to_string_lossy().into_owned()
}

#[test]
fn one_symbol_selects_every_instance_through_svsetscope() {
    let directory = sim_harness::TempDir::new("sim041").expect("temporary directory");
    let Some(library) = companion(&directory, "scopes") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "scopes",
        include_str!("../fixtures/sim/feature_completion/sim_041/scopes.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

/// The companion linked as a static archive gives the same output: exports
/// and scope routines need no dynamic symbol lookup from the library.
#[test]
fn static_archive_companion_reaches_exports_and_scopes() {
    let directory = sim_harness::TempDir::new("sim041-static").expect("temporary directory");
    let archive = static_companion(&directory, "scopes");
    sim_cli::run_case_backend_parity(
        SUITE,
        "scopes",
        include_str!("../fixtures/sim/feature_completion/sim_041/scopes.out"),
        &["--dpi-lib", &archive],
        &[],
    );
}

#[test]
fn chains_reenter_repeatedly_without_advancing_time() {
    let directory = sim_harness::TempDir::new("sim041").expect("temporary directory");
    let Some(library) = companion(&directory, "chains") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "chains",
        include_str!("../fixtures/sim/feature_completion/sim_041/chains.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

/// `byte` formals and results of an export are C `char` (Table H.1), whose
/// signedness is the ABI's. `-funsigned-char` on the companion and the model
/// reproduces the AArch64 Linux ABI; SystemVerilog must see the same values.
#[test]
fn export_bytes_keep_their_sign_when_char_is_unsigned() {
    let directory = sim_harness::TempDir::new("sim041-uchar").expect("temporary directory");
    let Some(library) =
        super::sim_040::shared_companion(SUITE, &directory, "chains", &["-funsigned-char"])
    else {
        return;
    };
    let cflags = std::env::var("LLG_CFLAGS").map_or_else(
        |_| "-funsigned-char".to_owned(),
        |flags| format!("{flags} -funsigned-char"),
    );
    let output = sim_cli::invoke_with_env(
        SUITE,
        "chains",
        true,
        &["--dpi-lib", &library],
        &[("LLG_CFLAGS", &cflags)],
        &[],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        include_str!("../fixtures/sim/feature_completion/sim_041/chains.out")
    );
}

#[test]
fn disables_follow_the_dpi_protocol() {
    let directory = sim_harness::TempDir::new("sim041").expect("temporary directory");
    let Some(library) = companion(&directory, "disables") else {
        return;
    };
    sim_cli::run_case_backend_parity(
        SUITE,
        "disables",
        include_str!("../fixtures/sim/feature_completion/sim_041/disables.out"),
        &["--dpi-lib", &library],
        &[],
    );
}

/// The check of a protocol violation: a fatal error after the output that
/// precedes it (SV 35.9: "a fatal simulation error is issued"; 35.5.3 and
/// 35.8 leave the behaviour to the implementation).
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

#[test]
fn protocol_violations_are_fatal_errors() {
    let directory = sim_harness::TempDir::new("sim041-errors").expect("temporary directory");
    let Some(library) = companion(&directory, "errors") else {
        return;
    };
    let args = ["--dpi-lib", library.as_str()];
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_noncontext_export",
        &args,
        &fatal_after(
            "start\n",
            "llg: DPI-C export `err_inc` was called from a noncontext import; only a context import call chain may call exported subroutines (SV 35.5.3)",
        ),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_scope_without_export",
        &args,
        &fatal_after(
            "start\nc err_scope kept=held!\n",
            "llg: DPI-C export `err_inc` is not exported from the current scope `tb.uq`; call svSetScope with a scope that exports it (SV 35.5.3)",
        ),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_task_from_function",
        &args,
        &fatal_after(
            "start\n",
            "llg: DPI-C export task `err_tick` was called from an imported function; only imported tasks may call exported tasks (SV 35.8)",
        ),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_missing_ack",
        &args,
        &fatal_after(
            "start\n",
            "llg: DPI-C import function `err_no_ack` returned from a disable without calling svAckDisabledState (SV 35.9 item c)",
        ),
    );
    sim_cli::run_case_checked_matrix(
        SUITE,
        "neg_call_after_disable",
        &args,
        &fatal_after(
            "start\n",
            "llg: DPI-C export `err_inc` was called after a disable cancelled the calling import (SV 35.9 item d)",
        ),
    );
}

#[test]
fn library_call_to_an_undeclared_export_is_named_at_link() {
    let directory = sim_harness::TempDir::new("sim041-missing").expect("temporary directory");
    let Some(library) = companion(&directory, "errors") else {
        return;
    };
    sim_cli::reject_case_with_args(
        SUITE,
        "neg_missing_export",
        "a DPI-C library calls `err_kill`, `err_tick`, which no export \"DPI-C\" declaration of the design and no --dpi-lib library defines",
        &["--dpi-lib", &library],
    );
}

#[test]
fn unsupported_exports_are_codegen_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_timed_export_task",
        "DPI-C export task `wait_one` (C identifier `wait_one`) can consume time; exported tasks that suspend are not supported (ADV-018)",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_reserved_c_name",
        "DPI-C export `get_scope` uses C identifier `svGetScope`, which the generated model or its runtime defines",
    );
}

#[test]
fn illegal_exports_are_frontend_errors() {
    sim_cli::reject_case(
        SUITE,
        "neg_duplicate_c_name",
        "more than one DPI export with the same C identifier 'one' in the same scope",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_conflicting_export",
        "more than one DPI subroutine with C identifier 'same' declared with mismatching type signatures",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_import_and_export",
        "more than one DPI subroutine with C identifier 'twice' declared with mismatching type signatures",
    );
    sim_cli::reject_case(
        SUITE,
        "neg_undeclared_export",
        "use of undeclared identifier 'missing_fn'",
    );
}
