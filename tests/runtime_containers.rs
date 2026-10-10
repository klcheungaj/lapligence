//! Standalone coverage for scheduler-independent C container storage.

use std::time::Duration;

use crate::{c_compiler, sim_harness};

const CONTAINER_PROBE: &str = include_str!("runtime_value_storage/container_isolation_probe.c");

#[test]
fn container_runtime_compiles_and_runs_without_scheduler() {
    let compiler = c_compiler::host_c_compiler();
    if !c_compiler::c_compiler_available(&compiler) {
        eprintln!("SKIP: C compiler `{compiler}` not available");
        return;
    }

    let dir = sim_harness::TempDir::new("runtime-containers").expect("create temp directory");
    sim_harness::write_value_backend_sources(dir.path());
    let (value_header, value_implementation) = llg::sim::rt::value_sources();
    let (rng_header, rng_implementation) = llg::sim::rt::rng_sources();
    let (string_header, string_implementation) = llg::sim::rt::string_sources();
    let (container_header, container_implementation) = llg::sim::rt::container_sources();
    for (name, contents) in [
        ("llg_value.h", value_header),
        ("llg_value.c", value_implementation),
        ("llg_rng.h", rng_header),
        ("llg_rng.c", rng_implementation),
        ("llg_string.h", string_header),
        ("llg_string.c", string_implementation),
        ("llg_container.h", container_header),
        ("llg_container.c", container_implementation),
        ("runtime_containers_probe.c", CONTAINER_PROBE),
    ] {
        std::fs::write(dir.path().join(name), contents).expect("write runtime source");
    }

    std::fs::write(
        dir.path().join("test_value_temporaries.h"),
        include_str!("runtime_value_storage/test_value_temporaries.h"),
    )
    .expect("write test owner helper");

    let (mut command, executable) = c_compiler::strict_c11_executable(
        &compiler,
        dir.path(),
        c_compiler::Optimize::Speed,
        &[
            "llg_value.c",
            "llg_rng.c",
            "llg_string.c",
            "llg_container.c",
            "runtime_containers_probe.c",
        ],
        "runtime_containers_probe",
    );
    let compiled = sim_harness::run_command(&mut command, Duration::from_secs(60))
        .unwrap_or_else(|error| panic!("run C compiler `{compiler}`: {error}"));
    assert!(
        compiled.status.success(),
        "standalone container runtime must compile:\n{}",
        c_compiler::compiler_diagnostics(&compiled)
    );

    let stdout = sim_harness::run_executable(&executable).expect("container probe should run");
    assert_eq!(stdout, "runtime container isolation ok\n");
}
