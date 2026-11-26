//! The C++ options build.rs gives the native frontend
//! (build_support/native_cxx.rs) against the wrapper's guard, compiled by the
//! compiler that builds models on this host: CI runs it under GCC, Clang,
//! Apple Clang and MSVC.

#[allow(clippy::module_inception)]
#[path = "../build_support/native_cxx.rs"]
mod native_cxx;

use std::process::{Command, Output};
use std::time::Duration;

use crate::{c_compiler, sim_harness};

/// A C++ translation unit that includes `src/wrapper/slang_platform.hpp`,
/// checked by the host compiler in the wrapper's C++20 with the frontend's
/// options for its kind of compiler, or with exceptions turned off.
fn check_wrapper_header(exceptions: bool) -> Output {
    let compiler = c_compiler::host_c_compiler();
    let msvc = c_compiler::is_msvc(&compiler);
    let directory = sim_harness::TempDir::new("native-cxx").expect("scratch");
    let unit = directory.path().join("probe.cpp");
    std::fs::write(&unit, "#include \"slang_platform.hpp\"\n").expect("write the probe");
    let wrapper = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/wrapper");
    let mut command = Command::new(&compiler);
    if msvc {
        command
            .args(["/nologo", "/TP", "/std:c++20", "/Zs"])
            .arg("/I")
            .arg(&wrapper);
        if exceptions {
            command.args(native_cxx::cxx_flags("msvc"));
        }
    } else {
        command
            .args(["-x", "c++", "-std=c++20", "-fsyntax-only", "-I"])
            .arg(&wrapper);
        if !exceptions {
            command.arg("-fno-exceptions");
        }
    }
    sim_harness::run_command(command.arg(&unit), Duration::from_secs(60))
        .expect("run the host compiler")
}

#[test]
fn component_msvc_and_clang_cl_get_exceptions_and_rtti() {
    assert_eq!(native_cxx::cxx_flags("msvc"), ["/EHsc", "/GR"]);
    for target_env in ["gnu", "musl", ""] {
        assert!(native_cxx::cxx_flags(target_env).is_empty(), "{target_env}");
    }
}

/// The frontend's options compile the wrapper; without exception handling,
/// in which Slang aborts where it would throw, the wrapper stops the build.
#[test]
fn component_the_wrapper_stops_a_build_without_exceptions() {
    let built = check_wrapper_header(true);
    assert!(
        built.status.success(),
        "{}",
        c_compiler::compiler_diagnostics(&built)
    );
    let refused = check_wrapper_header(false);
    let diagnostics = c_compiler::compiler_diagnostics(&refused);
    assert!(!refused.status.success(), "{diagnostics}");
    assert!(
        diagnostics.contains("need C++ exception handling"),
        "{diagnostics}"
    );
}
