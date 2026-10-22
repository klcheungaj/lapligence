//! llg-specific policy cases. Every fixture under
//! `tests/fixtures/sim/llg_policy/` pins a choice that llg makes where the
//! LRM leaves the result open and that has no portable form: an RNG draw
//! mapping, the draws a failed randomize() makes, the spelling of `%m` for class scopes, and the results of NULL
//! arguments of the DPI scope routines. They are not in the portable
//! `lrm_decisions` set, so they are not meant to be rerun on another
//! simulator. Each runs through the public CLI in both optimizer modes on
//! the legacy, compact/portable and compact/GMP value backends, like the
//! portable cases.
use crate::sim_cli;

const SUITE: &str = "llg_policy";

/// Exact stdout on every backend and optimizer mode, with identical stderr
/// across backends.
fn run_policy(case: &str, expected: &str) {
    sim_cli::run_case_backend_parity(SUITE, case, expected, &[], &[]);
}

/// [`run_policy`] for a case with a DPI-C `.c` companion. See the portable
/// suite's `dpi_companion` for the Windows import-library handling.
fn run_dpi_policy(case: &str, expected: &str) {
    let directory = crate::sim_harness::TempDir::new("llg-policy").expect("temporary directory");
    if cfg!(windows) && !crate::c_compiler::is_msvc(&crate::c_compiler::host_c_compiler()) {
        eprintln!("SKIP: Windows DLL companions are built with MSVC only");
        return;
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixtures = root.join("tests/fixtures/sim").join(SUITE);
    let import = cfg!(windows).then(|| {
        sim_cli::model_import_library(&fixtures.join(format!("{case}.sv")), &[], directory.path())
    });
    let library = crate::c_compiler::shared_library(
        &fixtures.join(format!("{case}.c")),
        directory.path(),
        case,
        &[],
        import.as_deref(),
    )
    .unwrap_or_else(|error| panic!("{case}.c: {error}"));
    let library = library.link.to_string_lossy().into_owned();
    sim_cli::run_case_backend_parity(SUITE, case, expected, &["--dpi-lib", &library], &[]);
}

#[test]
fn s16_d9_draw_is_urandom_range() {
    run_policy(
        "S16-D9_draw_is_urandom_range",
        include_str!("fixtures/sim/llg_policy/S16-D9_draw_is_urandom_range.out"),
    );
}

#[test]
fn s29_d6_failed_call_consumes_draws() {
    run_policy(
        "S29-D6_failed_call_consumes_draws",
        include_str!("fixtures/sim/llg_policy/S29-D6_failed_call_consumes_draws.out"),
    );
}

#[test]
fn s31_d7_draw_is_urandom_range() {
    run_policy(
        "S31-D7_draw_is_urandom_range",
        include_str!("fixtures/sim/llg_policy/S31-D7_draw_is_urandom_range.out"),
    );
}

#[test]
fn s99_d10_full_scope_names() {
    run_policy(
        "S99-D10_full_scope_names",
        include_str!("fixtures/sim/llg_policy/S99-D10_full_scope_names.out"),
    );
}

#[test]
fn cd_d2_full_scope_names() {
    run_policy(
        "CD-D2_full_scope_names",
        include_str!("fixtures/sim/llg_policy/CD-D2_full_scope_names.out"),
    );
}

#[test]
fn s41_d4_null_arguments() {
    run_dpi_policy(
        "S41-D4_null_arguments",
        include_str!("fixtures/sim/llg_policy/S41-D4_null_arguments.out"),
    );
}
