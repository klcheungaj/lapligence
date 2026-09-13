//! H27 DPI-C import acceptance tests.
//!
//! The HDL and foreign C sources are checked-in fixtures. Rust owns the
//! temporary library build, CLI orchestration, exact stdout oracle, and
//! diagnostics for the negative cases. Native model builds are skipped when
//! CMake or a C compiler is unavailable, matching the other simulator suites.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use llg::core::{compile, db::Db, model::DesignModel};
use llg::sim;

#[path = "support/sim.rs"]
mod sim_harness;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/sim/dpi")
        .join(name)
}

fn run_llg(directory: &Path, source: &Path, library: Option<&Path>, no_opt: bool) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_llg"));
    command.current_dir(directory).args(["--top", "tb"]);
    if no_opt {
        command.arg("--no-opt");
    }
    if let Some(library) = library {
        command.args(["--dpi-lib", &library.to_string_lossy()]);
    }
    command.arg(source);
    sim_harness::run_command(&mut command, std::time::Duration::from_secs(180))
        .expect("llg should start")
}

fn compile_shared_library(directory: &Path, source: &Path, name: &str) -> Option<PathBuf> {
    #[cfg(not(unix))]
    {
        let _ = (directory, source, name);
        return None;
    }
    #[cfg(unix)]
    {
        let compiler = std::env::var("LLG_CC")
            .or_else(|_| std::env::var("CC"))
            .unwrap_or_else(|_| "cc".to_owned());
        let output = directory.join(format!("lib{name}.so"));
        let include = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/slang/external/ieee1800");
        let mut command = Command::new(compiler);
        command
            .args(["-shared", "-fPIC"])
            .arg("-I")
            .arg(include)
            .arg(source)
            .arg("-o")
            .arg(&output);
        let result =
            match sim_harness::run_command(&mut command, std::time::Duration::from_secs(30)) {
                Ok(result) => result,
                Err(error) => {
                    eprintln!("SKIP: C compiler cannot build DPI fixture: {error}");
                    return None;
                }
            };
        if !result.status.success() {
            eprintln!(
                "SKIP: C compiler cannot build DPI fixture: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            return None;
        }
        Some(output)
    }
}

const EXPECTED_ROUNDTRIP: &str = "int=17,17 task=38 sum=15\n\
logic=x/x/z reg=x bit=1\n\
byte=-2 u64=18446744073709551613\n\
real=1.750000 io=2.500000/5.000000\n\
shortreal=ok\n\
handle_null=1 io=1/0 string=dpi-ok io=dpi-out/dpi-inout\n";

#[test]
fn dpi_scalar_imports_roundtrip_in_both_optimizer_modes() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    #[cfg(not(unix))]
    {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return;
    }

    let directory = sim_harness::TempDir::new("dpi-roundtrip").expect("temporary directory");
    let source = fixture("roundtrip.sv");
    let c_source = fixture("roundtrip.c");
    let Some(library) = compile_shared_library(directory.path(), &c_source, "dpi_roundtrip") else {
        return;
    };

    for no_opt in [true, false] {
        let output = run_llg(directory.path(), &source, Some(&library), no_opt);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "no_opt={no_opt}: stderr: {stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), EXPECTED_ROUNDTRIP);
        assert!(
            !stderr
                .lines()
                .any(|line| line.starts_with("llg: warning: ")),
            "unexpected lowering warning: {stderr}"
        );
    }

    let model = directory.path().join("target/sim/tb/model.c");
    let model_c = std::fs::read_to_string(&model).expect("generated model source");
    assert!(model_c.contains("#include \"svdpi.h\""));
    assert!(model_c.contains("extern int32_t dpi_add(int32_t p0, int32_t p1);"));
    assert!(model_c.contains("extern svLogic dpi_logic(svLogic p0);"));
    assert!(model_c.contains("extern svLogic dpi_reg(svLogic p0);"));
    assert!(model_c.contains("c_name=pure_add context=0 pure=1"));
    assert!(model_c.contains("c_name=context_add context=1 pure=0"));
}

#[test]
fn dpi_metadata_is_owned_and_keeps_alias_and_qualifiers() {
    let source = fixture("roundtrip.sv");
    let compiled = compile::compile_checked(&compile::CompileOpts {
        files: vec![source.to_string_lossy().into_owned()],
        top: Some("tb".to_owned()),
        ..Default::default()
    })
    .expect("DPI fixture should compile");
    let db = Db::from_slang(&compiled.snapshot).expect("owned database");
    let model = DesignModel::from_db(&db);
    let top = model.instance("tb").expect("top instance");

    let pure = top.func("pure_add").expect("pure import");
    assert_eq!(
        pure.dpi_import.as_ref().map(|dpi| dpi.c_name.as_str()),
        Some("pure_add")
    );
    assert!(pure.dpi_import.as_ref().is_some_and(|dpi| dpi.pure));
    assert!(!pure.dpi_import.as_ref().is_some_and(|dpi| dpi.context));

    let context = top.func("context_add").expect("context import");
    assert_eq!(
        context.dpi_import.as_ref().map(|dpi| dpi.c_name.as_str()),
        Some("context_add")
    );
    assert!(context.dpi_import.as_ref().is_some_and(|dpi| dpi.context));
    assert!(!context.dpi_import.as_ref().is_some_and(|dpi| dpi.pure));
}

#[test]
fn dpi_missing_symbol_fails_during_model_link() {
    if !sim::build::cmake_available() {
        eprintln!("SKIP: cmake not available");
        return;
    }
    #[cfg(not(unix))]
    {
        eprintln!("SKIP: shared DPI fixture build is only enabled on Unix hosts");
        return;
    }

    let directory = sim_harness::TempDir::new("dpi-missing").expect("temporary directory");
    let Some(library) =
        compile_shared_library(directory.path(), &fixture("missing.c"), "dpi_missing")
    else {
        return;
    };
    let output = run_llg(
        directory.path(),
        &fixture("roundtrip.sv"),
        Some(&library),
        true,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(output.stdout.is_empty(), "unexpected simulation output");
    assert!(stderr.contains("cmake build failed"), "stderr: {stderr}");
    assert!(!stderr.contains("llg: $finish"), "simulation ran: {stderr}");
}

#[test]
fn dpi_conflicting_aliases_fail_before_codegen() {
    let directory = sim_harness::TempDir::new("dpi-conflict").expect("temporary directory");
    let output = run_llg(
        directory.path(),
        &fixture("conflicting_signature.sv"),
        None,
        false,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(output.stdout.is_empty());
    assert!(
        stderr.contains("mismatching type signatures"),
        "conflict was not diagnosed: {stderr}"
    );
}

#[test]
fn dpi_packed_vector_is_rejected_at_the_scalar_boundary() {
    let directory = sim_harness::TempDir::new("dpi-vector").expect("temporary directory");
    let output = run_llg(
        directory.path(),
        &fixture("unsupported_vector.sv"),
        None,
        false,
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "stderr: {stderr}");
    assert!(output.stdout.is_empty());
    assert!(
        stderr.contains("outside the supported scalar ABI"),
        "boundary rejection missing: {stderr}"
    );
}

#[test]
fn dpi_library_options_are_explicit_and_prevalidated() {
    let directory = sim_harness::TempDir::new("dpi-options").expect("temporary directory");
    let library = directory.path().join("libdpi.a");
    std::fs::write(&library, "placeholder").expect("placeholder library");
    let opts = sim::build::CmakeBuildOpts {
        dpi_libraries: vec![library.clone()],
        ..Default::default()
    };
    sim::build::generate_model_sources_with_opts(
        directory.path(),
        &[("model.c", "int main(void) { return 0; }\n")],
        &opts,
    )
    .expect("valid regular library path");
    let cmake = std::fs::read_to_string(directory.path().join("CMakeLists.txt"))
        .expect("generated CMake file");
    let cmake_path = library.to_string_lossy().replace('\\', "/");
    assert!(
        cmake.contains(&format!(
            "target_link_libraries(sim PRIVATE \"{cmake_path}\")"
        )),
        "explicit library missing from CMake: {cmake}"
    );

    let missing = directory.path().join("does-not-exist.so");
    let bad_opts = sim::build::CmakeBuildOpts {
        dpi_libraries: vec![missing.clone()],
        ..Default::default()
    };
    let error = sim::build::generate_model_sources_with_opts(
        directory.path(),
        &[("model.c", "int main(void) { return 0; }\n")],
        &bad_opts,
    )
    .expect_err("missing library must fail before generation");
    assert!(matches!(
        error,
        sim::build::BuildError::InvalidDpiLibrary { path, .. } if path == missing
    ));
}
