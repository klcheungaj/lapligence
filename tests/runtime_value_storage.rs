//! Native storage/ownership checks without compiling a generated HDL model.

use crate::sim_harness;

use std::path::Path;
use std::process::Command;
use std::time::Duration;

const STAGE_TIMEOUT: Duration = Duration::from_secs(180);
const BUILD_TIMEOUT: Duration = Duration::from_secs(600);
/// The ctest stage runs about 120 probe processes in sequence, each bounded by
/// its own ctest TIMEOUT, so this cap only has to stop a run whose probes are
/// all slow. Linux runs the legacy stage in about 17 s. Windows CI exceeded
/// 180 s on both runners: every probe is a new process (Defender scans each
/// on arm64), the expect-failure and mixed-mode cases start `cmake -P` and
/// `cl`, and MSVC Debug probes run at `/Od /RTC1`. 600 s matches the build
/// stage and leaves over 3x the observed 180 s lower bound.
const TEST_STAGE_TIMEOUT: Duration = if cfg!(windows) {
    Duration::from_secs(600)
} else {
    STAGE_TIMEOUT
};

#[test]
fn dynamic_storage_and_waveform_snapshots() {
    run_storage_tests("runtime-value-storage", &[]);
}

/// The same runtime, scheduler and waveform probes against the compact backend
/// with portable kernels, as generated models select it.
#[test]
fn dynamic_storage_and_waveform_snapshots_compact_portable() {
    run_storage_tests(
        "runtime-value-storage-compact",
        &["-DLLG_STORAGE_TEST_VALUE_BACKEND=compact".to_owned()],
    );
}

/// The compact probes with GMP kernels: the bundled GMP, or the installation
/// named by `LLG_TEST_GMP_ROOT`.
#[test]
fn dynamic_storage_and_waveform_snapshots_compact_gmp() {
    run_storage_tests(
        "runtime-value-storage-gmp",
        &[
            "-DLLG_STORAGE_TEST_VALUE_BACKEND=compact".to_owned(),
            "-DLLG_STORAGE_TEST_COMPACT_KERNELS=gmp".to_owned(),
            format!("-DLLG_GMP_ROOT={}", sim_harness::test_gmp_root()),
        ],
    );
}

fn run_storage_tests(label: &str, options: &[String]) {
    let cmake = std::env::var("LLG_CMAKE").unwrap_or_else(|_| "cmake".to_owned());
    if Command::new(&cmake).arg("--version").output().is_err() {
        eprintln!("SKIP: CMake `{cmake}` not available");
        return;
    }
    let dir = sim_harness::TempDir::new(label).expect("create storage test directory");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/runtime_value_storage");
    let mut configure = Command::new(&cmake);
    configure
        .arg("-S")
        .arg(source)
        .arg("-B")
        .arg(dir.path())
        .arg("-DCMAKE_BUILD_TYPE=Debug")
        // The waveform probes compile the libfst sources the runtime embeds:
        // vendor/libfst, patched in place by the build script.
        .args(options);
    if let Ok(compiler) = std::env::var("LLG_CC").or_else(|_| std::env::var("CC")) {
        configure.arg(format!("-DCMAKE_C_COMPILER={compiler}"));
    }
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        configure.arg(format!("-DCMAKE_C_FLAGS:STRING={flags}"));
    }
    // Same launcher variable as generated models (`sim::build`), so a compiler
    // cache serves these probe builds too.
    if let Some(launcher) = std::env::var("LLG_C_LAUNCHER")
        .ok()
        .filter(|launcher| !launcher.trim().is_empty())
    {
        configure.arg(format!("-DCMAKE_C_COMPILER_LAUNCHER={}", launcher.trim()));
    }
    let mut build = Command::new(&cmake);
    build
        .arg("--build")
        .arg(dir.path())
        .args(["--config", "Debug"]);
    let ctest = Path::new(&cmake).with_file_name(format!("ctest{}", std::env::consts::EXE_SUFFIX));
    let mut test = Command::new(ctest);
    test.arg("--test-dir")
        .arg(dir.path())
        .args(["--build-config", "Debug", "--output-on-failure"]);
    for (stage, command, timeout) in [
        ("configure", &mut configure, STAGE_TIMEOUT),
        ("build", &mut build, BUILD_TIMEOUT),
        ("test", &mut test, TEST_STAGE_TIMEOUT),
    ] {
        let output = sim_harness::run_command(command, timeout)
            .unwrap_or_else(|error| panic!("storage tests {stage}: {error}"));
        assert!(
            output.status.success(),
            "storage tests {stage}:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
