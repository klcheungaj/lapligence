//! Host C compiler identification for native compile gates.
// Shared by several test groups; each uses a subset.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

/// Compiler for tests that compile runtime C directly: `$LLG_CC`, then `$CC`
/// (empty values are unset), then the generated-model default (MSVC `cl` on
/// Windows, `cc` elsewhere), so direct probes use the same compiler as models.
pub(crate) fn host_c_compiler() -> String {
    ["LLG_CC", "CC"]
        .into_iter()
        .find_map(|name| std::env::var(name).ok().filter(|value| !value.is_empty()))
        .unwrap_or_else(|| llg::sim::build::DEFAULT_C_COMPILER.to_owned())
}

/// True when `compiler` takes MSVC-style options (`cl`, `clang-cl`), judged
/// from its file name as CMake and ccache do.
pub(crate) fn is_msvc(compiler: &str) -> bool {
    let stem = Path::new(compiler)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    stem == "cl" || stem == "clang-cl"
}

/// Whether `compiler` can be started. MSVC `cl` has no version option (run
/// bare, it prints its banner and usage), so only a missing program counts.
pub(crate) fn c_compiler_available(compiler: &str) -> bool {
    let mut command = Command::new(compiler);
    if !is_msvc(compiler) {
        command.arg("--version");
    }
    command.stdin(Stdio::null()).output().is_ok()
}

/// Optimization of a [`strict_c11_executable`] build.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Optimize {
    None,
    Speed,
}

/// A command that builds `sources` (relative to `directory`, which is also the
/// include directory) into one strict C11 executable named `name` plus the
/// platform executable suffix, returned with the command. GNU-like compilers
/// get `-Wall -Wextra -Werror` and `-lm`; MSVC gets `/W4 /WX` like the
/// `runtime_value_storage` CMake probes. `$LLG_CFLAGS` follows the warnings,
/// as for generated models.
pub(crate) fn strict_c11_executable(
    compiler: &str,
    directory: &Path,
    optimize: Optimize,
    sources: &[&str],
    name: &str,
) -> (Command, PathBuf) {
    let executable = directory.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    let mut command = Command::new(compiler);
    command.current_dir(directory);
    let msvc = is_msvc(compiler);
    if msvc {
        let level = match optimize {
            Optimize::None => "/Od",
            Optimize::Speed => "/O2",
        };
        command.args([
            "/nologo",
            "/std:c11",
            level,
            "/W4",
            "/WX",
            "/D_CRT_SECURE_NO_WARNINGS",
            "/I.",
        ]);
    } else {
        let level = match optimize {
            Optimize::None => "-O0",
            Optimize::Speed => "-O2",
        };
        command.args(["-std=c11", level, "-Wall", "-Wextra", "-Werror", "-I."]);
    }
    if let Ok(flags) = std::env::var("LLG_CFLAGS") {
        command.args(flags.split_whitespace());
    }
    command.args(sources);
    if msvc {
        command.arg(format!("/Fe{}", executable.display()));
    } else {
        command.arg("-lm").arg("-o").arg(&executable);
    }
    (command, executable)
}

/// True when `compiler` is GNU GCC rather than a Clang driver installed under
/// the same name (macOS `gcc` is Apple Clang). GCC-only diagnostics such as
/// `-Werror=jump-misses-init` must be requested only from real GCC: Clang
/// reports them as unknown warning options. Probed once per name per process.
pub(crate) fn is_gnu_gcc(compiler: &str) -> bool {
    static PROBED: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let probed = PROBED.get_or_init(Default::default);
    if let Some(&known) = probed
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(compiler)
    {
        return known;
    }
    let gnu = probe_gnu_gcc(compiler);
    probed
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(compiler.to_owned(), gnu);
    gnu
}

fn probe_gnu_gcc(compiler: &str) -> bool {
    let Ok(output) = Command::new(compiler)
        .args(["-dM", "-E", "-x", "c", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let macros = String::from_utf8_lossy(&output.stdout);
    let defined = |name: &str| {
        macros
            .lines()
            .any(|line| line.split_whitespace().nth(1) == Some(name))
    };
    defined("__GNUC__") && !defined("__clang__")
}

/// The compiler ID CMake reports for a GNU-like `compiler`
/// (`CMAKE_C_COMPILER_ID`), judged from its predefined macros: `GNU` for real
/// GCC, `AppleClang` for Apple's Clang (which macOS installs as both `clang`
/// and `gcc`) and `Clang` for any other Clang. `None` when the macros cannot
/// be read or name neither family.
pub(crate) fn cmake_compiler_id(compiler: &str) -> Option<&'static str> {
    let output = Command::new(compiler)
        .args(["-dM", "-E", "-x", "c", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let macros = String::from_utf8_lossy(&output.stdout);
    let defined = |name: &str| {
        macros
            .lines()
            .any(|line| line.split_whitespace().nth(1) == Some(name))
    };
    if defined("__clang__") {
        Some(if defined("__apple_build_version__") {
            "AppleClang"
        } else {
            "Clang"
        })
    } else if defined("__GNUC__") {
        Some("GNU")
    } else {
        None
    }
}
