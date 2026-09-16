//! Native storage/ownership checks without compiling a generated HDL model.

#[path = "support/sim.rs"]
mod sim_harness;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[test]
fn dynamic_storage_and_waveform_snapshots() {
    let cmake = std::env::var("LLG_CMAKE").unwrap_or_else(|_| "cmake".to_owned());
    if Command::new(&cmake).arg("--version").output().is_err() {
        eprintln!("SKIP: CMake `{cmake}` not available");
        return;
    }
    let dir =
        sim_harness::TempDir::new("runtime-value-storage").expect("create storage test directory");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/runtime_value_storage");
    let mut configure = Command::new(&cmake);
    configure
        .arg("-S")
        .arg(source)
        .arg("-B")
        .arg(dir.path())
        .arg("-DCMAKE_BUILD_TYPE=Debug");
    if let Ok(compiler) = std::env::var("LLG_CC").or_else(|_| std::env::var("CC")) {
        configure.arg(format!("-DCMAKE_C_COMPILER={compiler}"));
    }
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        configure.arg(format!("-DCMAKE_C_FLAGS:STRING={flags}"));
    }
    let mut build = Command::new(&cmake);
    build
        .arg("--build")
        .arg(dir.path())
        .args(["--config", "Debug"]);
    let ctest =
        Path::new(&cmake).with_file_name(format!("ctest{}", std::env::consts::EXE_SUFFIX));
    let mut test = Command::new(ctest);
    test.arg("--test-dir")
        .arg(dir.path())
        .args(["--build-config", "Debug", "--output-on-failure"]);
    for (stage, command) in [
        ("configure", &mut configure),
        ("build", &mut build),
        ("test", &mut test),
    ] {
        let output = sim_harness::run_command(command, Duration::from_secs(180))
            .unwrap_or_else(|error| panic!("storage tests {stage}: {error}"));
        assert!(
            output.status.success(),
            "storage tests {stage}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
