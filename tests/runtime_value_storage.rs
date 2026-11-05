//! Native storage/ownership checks without compiling a generated HDL model.

use crate::{c_compiler, sim_harness};

use std::path::Path;
use std::process::Command;
use std::time::Duration;

const STAGE_TIMEOUT: Duration = Duration::from_secs(180);
const BUILD_TIMEOUT: Duration = Duration::from_secs(600);
/// The compact probe set runs about 150 CPU-bound CTest cases in parallel
/// (the exhaustive family probes take about a minute each).
const COMPACT_TEST_STAGE_TIMEOUT: Duration = Duration::from_secs(900);

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

/// The production facade client, built unchanged against the legacy backend and
/// the compact backend with portable and GMP kernels, must print identical
/// results for every facade operation (`facade_compare.py`). Runs on every host
/// because the client is plain C11 with no linker wrapping.
#[test]
fn facade_operations_agree_between_legacy_and_compact_backends() {
    run_storage_stages_with_flags(
        "runtime-value-facade-compare",
        &compact_options(),
        "Debug",
        &["facade_compare_all"],
        &[],
        &["-R", "^facade_compare_"],
    );
}

/// Every standalone compact probe: in-process and bridge differentials against
/// the legacy backend, independent oracles, allocation counters and rejection
/// cases. They build only on hosts with ELF linker wrapping and a POSIX shell
/// for the CTest scripts, so this test skips Windows, where the project is run
/// by hand (tests/runtime_value_storage/readme.md, standalone compact backend).
#[cfg(not(windows))]
#[test]
fn compact_value_backend_differential_probes() {
    run_storage_stages_with_flags(
        "runtime-value-compact-probes",
        &compact_options(),
        "Debug",
        &[
            "compact_checks",
            "selected_legacy_facade",
            "selected_portable_facade",
            "selected_gmp_facade",
            "facade_compare_all",
        ],
        &[],
        &[
            "-R",
            "^(compact_|selected_|facade_compare_)",
            "-E",
            "benchmark_smoke",
        ],
    );
}

/// CMake options that turn on the standalone compact probe project.
fn compact_options() -> Vec<String> {
    vec![
        "-DLLG_STORAGE_TEST_COMPACT=ON".to_owned(),
        "-DLLG_STORAGE_TEST_WAVEFORMS=OFF".to_owned(),
        "-DLLG_STORAGE_TEST_SCHEDULER=OFF".to_owned(),
        format!("-DLLG_GMP_ROOT={}", sim_harness::test_gmp_root()),
    ]
}

/// The two targets that compile the whole scheduler runtime (`llg_rt.c` with
/// the debug coroutine checks) and the VPI layer (`llg_vpi.c`) under the strict
/// warning set, at Release optimization: GCC's `-O3` flow analysis reports
/// maybe-uninitialized reads that the Debug build above cannot see. Building
/// only these targets keeps the check far cheaper than a second full build.
///
/// Under GNU GCC the build adds [`GCC11_MAYBE_UNINITIALIZED_FLAGS`]; GCC 13
/// then reports what CI's GCC 11 and 13 do, GCC 14 less (docs/cross_platform.md,
/// Compiler versions).
#[test]
fn release_runtime_units_build_under_strict_warnings() {
    let gcc = c_compiler::is_gnu_gcc(&c_compiler::host_c_compiler());
    run_storage_stages_with_flags(
        "runtime-value-storage-release",
        &[],
        "Release",
        &["owner_probe_debug_rt", "vpi_ownership_probe"],
        if gcc {
            c_compiler::GCC11_MAYBE_UNINITIALIZED_FLAGS
        } else {
            &[]
        },
        &[],
    );
}

fn run_storage_tests(label: &str, options: &[String]) {
    run_storage_stages_with_flags(label, options, "Debug", &[], &[], &[]);
}

/// Configures, builds and runs the probes in build configuration `config`:
/// every probe when `targets` is empty, otherwise only `targets`, and then the
/// CTest cases selected by `test_args` (none when `test_args` is empty).
/// `extra_c_flags` are appended to `$LLG_CFLAGS` in `CMAKE_C_FLAGS`.
fn run_storage_stages_with_flags(
    label: &str,
    options: &[String],
    config: &str,
    targets: &[&str],
    extra_c_flags: &[&str],
    test_args: &[&str],
) {
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
        .arg(format!("-DCMAKE_BUILD_TYPE={config}"))
        .arg(format!(
            "-DLLG_STORAGE_TEST_DEBUG_INFO={}",
            if sim_harness::test_debug_info() {
                "ON"
            } else {
                "OFF"
            }
        ))
        // The waveform probes compile the libfst sources the runtime embeds:
        // vendor/libfst, patched in place by the build script.
        .args(options);
    if let Ok(compiler) = std::env::var("LLG_CC").or_else(|_| std::env::var("CC")) {
        configure.arg(format!("-DCMAKE_C_COMPILER={compiler}"));
    }
    let flags: Vec<String> = std::env::var("LLG_CFLAGS")
        .ok()
        .into_iter()
        .chain(extra_c_flags.iter().map(|flag| (*flag).to_owned()))
        .collect();
    if !flags.is_empty() {
        configure.arg(format!("-DCMAKE_C_FLAGS:STRING={}", flags.join(" ")));
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
        .args(["--config", config, "--parallel"])
        .arg(sim_harness::cmake_build_jobs());
    if !targets.is_empty() {
        build.arg("--target").args(targets);
    }
    let ctest = Path::new(&cmake).with_file_name(format!("ctest{}", std::env::consts::EXE_SUFFIX));
    let mut test = Command::new(ctest);
    test.arg("--test-dir")
        .arg(dir.path())
        .args(["--build-config", config, "--output-on-failure"]);
    if !targets.is_empty() {
        test.arg("--parallel")
            .arg(sim_harness::cmake_build_jobs())
            .args(test_args);
    }
    let mut stages = vec![
        ("configure", &mut configure, STAGE_TIMEOUT),
        ("build", &mut build, BUILD_TIMEOUT),
    ];
    if targets.is_empty() || !test_args.is_empty() {
        let timeout = if targets.is_empty() {
            TEST_STAGE_TIMEOUT
        } else {
            COMPACT_TEST_STAGE_TIMEOUT
        };
        stages.push(("test", &mut test, timeout));
    }
    for (stage, command, timeout) in stages {
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
