//! End-to-end coverage for the bounded generated-model VPI bridge.
//!
//! The plugins are intentionally compiled against the generated `vpi_user.h`,
//! so the test exercises the public ABI rather than an internal Rust helper.

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use llg::core::compile;
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

fn fixture(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/vpi")
        .join(name);
    fs::read_to_string(path).expect("VPI fixture should be readable")
}

fn compile_plugin(dir: &Path, source_name: &str) -> PathBuf {
    let source_path = dir.join(source_name);
    fs::write(&source_path, fixture(source_name)).expect("write VPI plugin source");
    let output_path = dir.join("libllg_vpi_test.so");
    let compiler = std::env::var("CC").unwrap_or_else(|_| "cc".to_owned());
    let output = Command::new(compiler)
        .args([
            "-std=c11", "-Wall", "-Wextra", "-Werror", "-fPIC", "-shared",
        ])
        .args(["-I", dir.to_str().expect("temporary path is UTF-8")])
        .args([
            source_path.to_str().expect("plugin path is UTF-8"),
            "-o",
            output_path.to_str().expect("plugin output path is UTF-8"),
        ])
        .output()
        .expect("compile VPI plugin");
    assert!(
        output.status.success(),
        "VPI plugin compiler failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output_path
}

fn build_model(
    dir: &Path,
    source_name: &str,
    source: &str,
    system_subroutines: &[&str],
) -> PathBuf {
    let source_path = dir.join(source_name);
    fs::write(&source_path, source).expect("write VPI HDL fixture");
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![source_path.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        system_subroutines: system_subroutines
            .iter()
            .map(|prototype| (*prototype).to_owned())
            .collect(),
        ..Default::default()
    })
    .expect("VPI HDL should compile");
    let database = llg::core::db::Db::from_slang(&compiled.snapshot)
        .expect("VPI HDL should import into the owned database");
    let generated = sim::codegen::generate(&database).expect("VPI model should lower");
    sim::build::build_model_cmake(dir, &[("model.c", generated.model_c.as_str())])
        .expect("VPI model should build")
}

fn run_plugin(_dir: &Path, executable: &Path, plugin: &Path) -> std::process::Output {
    let mut command = Command::new(executable);
    command.env("LLG_VPI_PLUGIN", plugin);
    sim_harness::run_command(&mut command, Duration::from_secs(60))
        .expect("generated model should run")
}

#[test]
fn vpi_plugin_registers_tasks_functions_and_walks_hierarchy() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("vpi-positive", |dir| {
        let executable = build_model(
            dir,
            "vpi_basic.sv",
            &fixture("vpi_basic.sv"),
            &[
                "task $vpi_probe(input logic value)",
                "function logic [4:0] $vpi_sized(input logic value)",
                "function real $vpi_real()",
            ],
        );
        let plugin = compile_plugin(dir, "vpi_plugin.c");
        let output = run_plugin(dir, &executable, &plugin);
        assert!(output.status.success(), "VPI simulation failed: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "compile-arg=3\ncompile-args=1\nsized-compile-arg=3\nsized-compile-args=1\nreal-compile-args=0\nvpi-start=0:0\nlookup=1/1 parent=tb vars=3\nmetadata=1\nnegative=LLG_VPI_UNSUPPORTED\nstale=LLG_VPI_HANDLE\nhdl=1/17/2.5\nvpi-end\n"
        );
        assert!(
            output.stderr.is_empty(),
            "unexpected VPI stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    })
    .expect("VPI positive fixture should complete");
}

#[test]
fn vpi_registration_and_callback_errors_are_reported() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("vpi-negative", |dir| {
        let executable = build_model(dir, "vpi_negative.sv", &fixture("vpi_negative.sv"), &[]);
        let plugin = compile_plugin(dir, "vpi_bad_registration.c");
        let output = run_plugin(dir, &executable, &plugin);
        assert!(
            output.status.success(),
            "negative VPI probe failed: {output:?}"
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "bad-registration=LLG_VPI_REGISTRATION\nbad-callback=LLG_VPI_CALLBACK\nbad-handle=LLG_VPI_HANDLE\nnegative-hdl\n"
        );
        assert!(
            output.stderr.is_empty(),
            "unexpected VPI stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    })
    .expect("VPI negative fixture should complete");
}

#[test]
fn vpi_vectors_are_simulator_owned_even_with_unspecified_request_storage() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("vpi-vector-ownership", |dir| {
        let executable = build_model(
            dir,
            "vpi_vector_ownership.sv",
            &fixture("vpi_vector_ownership.sv"),
            &["task $vpi_vector_probe(input logic [69:0] value)"],
        );
        let plugin = compile_plugin(dir, "vpi_vector_ownership.c");
        let output = run_plugin(dir, &executable, &plugin);
        assert!(output.status.success(), "VPI vector probe failed: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "vpi vector ownership ok\n"
        );
        assert!(output.stderr.is_empty(), "unexpected VPI stderr: {output:?}");
        Ok(())
    })
    .expect("VPI vector ownership fixture should complete");
}

#[test]
fn vpi_borrowed_handles_expire_at_compile_size_and_call_callback_exit() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("vpi-call-lifetime", |dir| {
        let executable = build_model(
            dir,
            "vpi_call_lifetime.sv",
            &fixture("vpi_call_lifetime.sv"),
            &["function logic [7:0] $vpi_lifetime(input logic [7:0] value)"],
        );
        let plugin = compile_plugin(dir, "vpi_call_lifetime.c");
        let output = run_plugin(dir, &executable, &plugin);
        assert!(output.status.success(), "VPI lifetime probe failed: {output:?}");
        assert_eq!(
            String::from_utf8_lossy(&output.stdout),
            "vpi borrowed handles ok 1\nresult=17\nvpi borrowed handles ok 2\nresult=17\n"
        );
        assert!(output.stderr.is_empty(), "unexpected VPI stderr: {output:?}");
        Ok(())
    })
    .expect("VPI callback lifetime fixture should complete");
}

#[test]
fn vpi_time_query_honors_requested_format_and_scope() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    sim_harness::with_frontend_temp_cwd("vpi-time-formats", |dir| {
        let executable = build_model(
            dir, "vpi_time_formats.sv", &fixture("vpi_time_formats.sv"),
            &["task $vpi_time_formats()"],
        );
        let plugin = compile_plugin(dir, "vpi_time_formats.c");
        let output = run_plugin(dir, &executable, &plugin);
        assert!(output.status.success(), "VPI time fixture failed: {output:?}");
        assert_eq!(output.stdout, b"vpi time formats ok\n");
        assert!(output.stderr.is_empty(), "unexpected VPI stderr: {output:?}");
        Ok(())
    }).expect("VPI time format fixture");
}
