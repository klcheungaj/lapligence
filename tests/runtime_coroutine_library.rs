//! Strict compile gates for the embedded stackless coroutine library.

#[path = "support/sim.rs"]
mod sim_harness;

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn run_compile(compiler: &str, command: &mut Command, description: &str) {
    let output = sim_harness::run_command(command, Duration::from_secs(60))
        .unwrap_or_else(|error| panic!("run C compiler `{compiler}` for {description}: {error}"));
    assert!(
        output.status.success(),
        "{compiler} failed {description}:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn compiler_available(compiler: &str) -> bool {
    match Command::new(compiler).arg("--version").output() {
        Ok(output) => {
            assert!(
                output.status.success(),
                "C compiler `{compiler}` is present but its version probe failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            true
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            eprintln!("SKIP: C compiler `{compiler}` not available");
            false
        }
        Err(error) => panic!("probe C compiler `{compiler}`: {error}"),
    }
}

fn strict_compile(
    compiler: &str,
    is_gcc: bool,
    directory: &Path,
    source: &str,
    output: PathBuf,
    definitions: &[&str],
) {
    let mut command = Command::new(compiler);
    command.current_dir(directory).args([
        "-std=c11",
        "-Wall",
        "-Wextra",
        "-Wpedantic",
        "-Werror",
        "-Wunused-function",
    ]);
    if is_gcc {
        command.arg("-Werror=jump-misses-init");
    }
    for definition in definitions {
        command.arg(format!("-D{definition}"));
    }
    command.args(["-I.", "-c", source, "-o"]).arg(output);
    run_compile(compiler, &mut command, source);
}

#[test]
fn coroutine_library_is_strict_c11_on_gcc_and_clang() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let directory = sim_harness::TempDir::new("runtime-coroutine-library")
        .expect("create coroutine compile directory");
    let (header, implementation) = llg::sim::rt::coroutine_sources();
    std::fs::write(directory.path().join("llg_co.h"), header).expect("write llg_co.h");
    std::fs::write(directory.path().join("llg_co.c"), implementation).expect("write llg_co.c");
    std::fs::write(
        directory.path().join("llg_co_include_only.c"),
        include_str!("runtime_value_storage/llg_co_include_only.c"),
    )
    .expect("write include-only translation unit");

    for (compiler, is_gcc) in [("gcc", true), ("clang", false)] {
        if !compiler_available(compiler) {
            continue;
        }
        for (variant, definitions) in [
            ("plain", &[][..]),
            ("debug", &["LLG_CO_DEBUG"][..]),
            ("host", &["LLG_CO_HOST_ALLOC"][..]),
            ("debug-host", &["LLG_CO_DEBUG", "LLG_CO_HOST_ALLOC"][..]),
        ] {
            strict_compile(
                compiler,
                is_gcc,
                directory.path(),
                "llg_co.c",
                directory
                    .path()
                    .join(format!("{compiler}-{variant}-library.o")),
                definitions,
            );
            strict_compile(
                compiler,
                is_gcc,
                directory.path(),
                "llg_co_include_only.c",
                directory
                    .path()
                    .join(format!("{compiler}-{variant}-include.o")),
                definitions,
            );
        }

        let mut runtime = Command::new(compiler);
        runtime
            .current_dir(root)
            .args([
                "-std=c11",
                "-O2",
                "-Wall",
                "-Wno-unused-function",
                "-Isrc/sim/rt",
                "-Ivendor/libaco",
                "-c",
                "src/sim/rt/llg_rt.c",
                "-o",
            ])
            .arg(directory.path().join(format!("{compiler}-llg-rt.o")));
        run_compile(compiler, &mut runtime, "llg_rt.c with coroutine host hooks");
    }
}
