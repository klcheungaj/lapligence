//! Linux/macOS stand-in for MSVC's unreachable-code warning on the runtime C
//! that the standalone probes build with `/W4 /WX`.
//!
//! MSVC at `/O2` inlines a `static` helper that ends in `abort()` and then
//! reports a statement after an unconditional call to it as unreachable
//! (C4702), which `/WX` turns into a failed probe build (CI round 6:
//! `llg_constraint.c(744)` and `(7643)`). GCC and Clang accept that code at
//! `-Wall -Wextra`. Clang's `-Wmissing-noreturn` names a helper that never
//! returns but is not declared `_Noreturn`, and `-Wunreachable-code-aggressive`
//! then names a statement after a call to it, which together approximate
//! C4702 (Clang does not report dead code that comes from a macro expansion,
//! such as `return NULL;`). Skipped without a Clang driver and on Windows,
//! where the probes themselves build with MSVC.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use crate::{c_compiler, sim_harness};

/// A Clang driver: the host compiler when it is Clang, else `clang` on PATH.
fn clang_driver() -> Option<String> {
    if cfg!(windows) {
        return None;
    }
    let host = c_compiler::host_c_compiler();
    if matches!(
        c_compiler::cmake_compiler_id(&host),
        Some("Clang" | "AppleClang")
    ) {
        return Some(host);
    }
    (c_compiler::c_compiler_available("clang")
        && c_compiler::cmake_compiler_id("clang").is_some_and(|id| id != "GNU"))
    .then(|| "clang".to_owned())
}

fn check_unreachable_code(clang: &str, directory: &Path, source: &str) {
    let mut command = Command::new(clang);
    command.current_dir(directory).args([
        "-std=c11",
        "-fsyntax-only",
        "-I.",
        "-Werror=missing-noreturn",
        "-Werror=unreachable-code-aggressive",
        source,
    ]);
    let output = sim_harness::run_command(&mut command, Duration::from_secs(120))
        .unwrap_or_else(|error| panic!("run `{clang}` on {source}: {error}"));
    assert!(
        output.status.success(),
        "{source}: a helper that never returns must be declared _Noreturn and no statement may \
         follow an unconditional call to it (MSVC /W4 C4702):\n{}",
        c_compiler::compiler_diagnostics(&output)
    );
}

/// Every runtime source a standalone `/W4 /WX` probe compiles: the values
/// (legacy backend, as the probes use), RNG, strings, containers, random
/// service and constraint solver.
#[test]
fn probe_built_runtime_sources_have_no_code_after_fatal_helpers() {
    let Some(clang) = clang_driver() else {
        eprintln!("SKIP: no Clang driver to approximate MSVC C4702");
        return;
    };
    let dir = sim_harness::TempDir::new("runtime-strict-sources").expect("create temp directory");
    sim_harness::write_value_backend_sources(dir.path());
    let (value_header, value_source) = llg::sim::rt::value_sources();
    let (rng_header, rng_source) = llg::sim::rt::rng_sources();
    let (string_header, string_source) = llg::sim::rt::string_sources();
    let (container_header, container_source) = llg::sim::rt::container_sources();
    let (random_header, random_source) = llg::sim::rt::random_sources();
    let (constraint_header, constraint_source) = llg::sim::rt::constraint_sources();
    let files = [
        ("llg_value.h", value_header),
        ("llg_value.c", value_source),
        ("llg_rng.h", rng_header),
        ("llg_rng.c", rng_source),
        ("llg_string.h", string_header),
        ("llg_string.c", string_source),
        ("llg_container.h", container_header),
        ("llg_container.c", container_source),
        ("llg_random.h", random_header),
        ("llg_random.c", random_source),
        ("llg_constraint.h", constraint_header),
        ("llg_constraint.c", constraint_source),
    ];
    for (name, contents) in files {
        std::fs::write(dir.path().join(name), contents).expect("write runtime source");
    }
    for source in files.iter().map(|(name, _)| *name) {
        if source.ends_with(".c") {
            check_unreachable_code(&clang, dir.path(), source);
        }
    }
}
