use std::path::Path;
use std::process::Command;

fn audit(arguments: &[&str]) {
    run_script("scripts/check_value_facade.py", arguments);
}

fn run_script(script: &str, arguments: &[&str]) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(root.join(script))
        .args(arguments)
        .output()
        .expect("Python is required for the packed-value facade source audit");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn runtime_and_emitted_values_use_the_neutral_facade() {
    audit(&[]);
}

#[test]
fn facade_audit_rejects_private_reads_writes_and_initializers() {
    audit(&["--self-test"]);
}

/// Every operation declared by the facade headers has a row in
/// `tests/runtime_value_storage/facade_comparisons.tsv` naming the test that
/// runs it on the legacy and on the compact backend and compares the results.
/// Adding an operation without such a test fails here.
#[test]
fn every_facade_operation_has_a_backend_comparison() {
    run_script("scripts/check_value_comparisons.py", &[]);
}

#[test]
fn comparison_guard_rejects_unlisted_and_stale_operations() {
    run_script("scripts/check_value_comparisons.py", &["--self-test"]);
}
