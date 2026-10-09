//! Portable LRM decision cases (`tests/fixtures/sim/lrm_decisions/`): one
//! plain IEEE 1800-2009 source per decision, with a header that quotes the
//! clause and states the decision, and the exact expected stdout. The same
//! sources are meant to be rerun on other simulators; here each runs through
//! the public CLI in both optimizer modes on the legacy and compact value
//! backends.
use crate::sim_cli;

const SUITE: &str = "lrm_decisions";

fn check(case: &str, expected: &str) {
    sim_cli::run_case_backend_parity(SUITE, case, expected, &[], &[]);
}

#[test]
fn b1_string_property_wait() {
    check(
        "B1_string_property_wait",
        include_str!("fixtures/sim/lrm_decisions/B1_string_property_wait.out"),
    );
}

#[test]
fn b2_function_member_wait() {
    check(
        "B2_function_member_wait",
        include_str!("fixtures/sim/lrm_decisions/B2_function_member_wait.out"),
    );
}

#[test]
fn b3_resume_resensitize() {
    check(
        "B3_resume_resensitize",
        include_str!("fixtures/sim/lrm_decisions/B3_resume_resensitize.out"),
    );
}

#[test]
fn b8_context_import_strobe() {
    if !cfg!(unix) {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return;
    }
    let directory = crate::sim_harness::TempDir::new("lrm-b8").expect("temporary directory");
    let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/lrm_decisions/B8_context_import_strobe.c");
    let library = directory.path().join("libb8_context.so");
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    let output = crate::sim_harness::run_command(
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
        "B8_context_import_strobe",
        include_str!("fixtures/sim/lrm_decisions/B8_context_import_strobe.out"),
        &["--dpi-lib", &library],
        &[],
    );
}
