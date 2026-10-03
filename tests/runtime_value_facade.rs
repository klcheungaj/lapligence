use std::path::Path;
use std::process::Command;

fn audit(arguments: &[&str]) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(if cfg!(windows) { "python" } else { "python3" })
        .arg(root.join("scripts/check_value_facade.py"))
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
