//! Standalone coverage for the scheduler-independent C value runtime.

use std::process::Command;
use std::time::Duration;

#[path = "support/sim.rs"]
mod sim_harness;

const VALUE_PROBE: &str = include_str!("runtime_value_storage/value_isolation_probe.c");

const VALUE_BOUNDARY_PROBE: &str = r#"
#include "llg_value.h"

#include <stdint.h>
#include <string.h>

#define OVER_CAP_WIDTH LLG_SUPPORTED_WIDTH_LIMIT

int main(int argc, char** argv) {
    if (argc != 2) return 2;
    if (strcmp(argv[1], "constructor") == 0) {
        (void)sv4_fill(1, OVER_CAP_WIDTH, 0);
        return 0;
    }
    if (strcmp(argv[1], "resolution") == 0) {
        sv4_t driver = sv4_fill(1, 1, 0);
        const sv4_t* drivers[1] = {&driver};
        (void)sv4_resolve(drivers, 1, OVER_CAP_WIDTH, 0, LLG_RESOLVE_WIRE);
        return 0;
    }
    return 2;
}
"#;

#[test]
fn value_runtime_compiles_and_runs_without_scheduler() {
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    if Command::new(&compiler).arg("--version").output().is_err() {
        eprintln!("SKIP: C compiler `{compiler}` not available");
        return;
    }

    let dir = sim_harness::TempDir::new("runtime-values").expect("create temp directory");
    let (header, implementation) = llg::sim::rt::value_sources();
    std::fs::write(dir.path().join("llg_value.h"), header).expect("write value header");
    std::fs::write(dir.path().join("llg_value.c"), implementation)
        .expect("write value implementation");
    std::fs::write(dir.path().join("runtime_values_probe.c"), VALUE_PROBE)
        .expect("write value probe");

    std::fs::write(
        dir.path().join("test_value_temporaries.h"),
        include_str!("runtime_value_storage/test_value_temporaries.h"),
    )
    .expect("write test owner helper");

    let executable = dir.path().join("runtime_values_probe");
    let mut command = Command::new(&compiler);
    command
        .current_dir(dir.path())
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-I."]);
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        command.args(flags.split_whitespace());
    }
    command
        .args(["llg_value.c", "runtime_values_probe.c", "-lm", "-o"])
        .arg(&executable);

    let compiled = sim_harness::run_command(&mut command, Duration::from_secs(60))
        .unwrap_or_else(|error| panic!("run C compiler `{compiler}`: {error}"));
    assert!(
        compiled.status.success(),
        "standalone value runtime must compile without scheduler/libaco:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    let stdout = sim_harness::run_executable(&executable).expect("value probe should run");
    assert_eq!(stdout, "runtime value isolation ok\n");
}

#[test]
fn value_runtime_rejects_over_capacity_widths() {
    let compiler = std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_owned());
    if Command::new(&compiler).arg("--version").output().is_err() {
        eprintln!("SKIP: C compiler `{compiler}` not available");
        return;
    }

    let dir = sim_harness::TempDir::new("runtime-value-boundaries").expect("create temp directory");
    let (header, implementation) = llg::sim::rt::value_sources();
    std::fs::write(dir.path().join("llg_value.h"), header).expect("write value header");
    std::fs::write(dir.path().join("llg_value.c"), implementation)
        .expect("write value implementation");
    std::fs::write(
        dir.path().join("runtime_values_boundary_probe.c"),
        VALUE_BOUNDARY_PROBE,
    )
    .expect("write value boundary probe");

    let executable = dir.path().join("runtime_values_boundary_probe");
    let mut command = Command::new(&compiler);
    command
        .current_dir(dir.path())
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-I."]);
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        command.args(flags.split_whitespace());
    }
    command
        .args([
            "llg_value.c",
            "runtime_values_boundary_probe.c",
            "-lm",
            "-o",
        ])
        .arg(&executable);

    let compiled = sim_harness::run_command(&mut command, Duration::from_secs(60))
        .unwrap_or_else(|error| panic!("run C compiler `{compiler}`: {error}"));
    assert!(
        compiled.status.success(),
        "standalone value boundary probe must compile:\n{}",
        String::from_utf8_lossy(&compiled.stderr)
    );

    for operation in ["constructor", "resolution"] {
        let mut command = Command::new(&executable);
        command.arg(operation);
        let output = sim_harness::run_command(&mut command, Duration::from_secs(10))
            .unwrap_or_else(|error| panic!("run {operation} boundary probe: {error}"));
        assert!(
            !output.status.success(),
            "over-capacity {operation} request must fail"
        );
        let diagnostic = String::from_utf8_lossy(&output.stderr);
        assert!(
            diagnostic.to_ascii_lowercase().contains("width"),
            "over-capacity {operation} failure must diagnose width: {diagnostic:?}"
        );
    }
}
