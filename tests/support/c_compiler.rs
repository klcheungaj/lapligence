//! Host C compiler identification for native compile gates.
// Shared by several test groups; each uses a subset.
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::{Mutex, OnceLock};

/// The environment variable `name`; an empty value is unset.
fn environment(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// Compiler for tests that compile runtime C directly: `$LLG_CC`, then `$CC`
/// (empty values are unset), then the generated-model default (MSVC `cl` on
/// Windows, `cc` elsewhere), so direct probes use the same compiler as models.
pub(crate) fn host_c_compiler() -> String {
    ["LLG_CC", "CC"]
        .into_iter()
        .find_map(environment)
        .unwrap_or_else(|| llg::sim::build::DEFAULT_C_COMPILER.to_owned())
}

/// A GNU GCC driver: the host compiler when it is GCC, else `gcc` on PATH
/// when that is GCC (macOS `gcc` is Apple Clang). None on Windows.
pub(crate) fn gcc_driver() -> Option<String> {
    if cfg!(windows) {
        return None;
    }
    let host = host_c_compiler();
    if is_gnu_gcc(&host) {
        return Some(host);
    }
    (c_compiler_available("gcc") && is_gnu_gcc("gcc")).then(|| "gcc".to_owned())
}

/// The environment variable naming the GCC of the heavier-inlining
/// maybe-uninitialized pass, which needs a newer GCC than many hosts' default.
/// No other test and no model build reads it.
pub(crate) const HEAVY_INLINING_GCC_VARIABLE: &str = "LLG_TEST_HEAVY_INLINING_GCC";

/// The oldest GCC the heavier-inlining pass runs under: GCC 13 reports false
/// positives for flags like these (`docs/cross_platform.md`, Compiler
/// versions).
const HEAVY_INLINING_MIN_GCC: u32 = 14;

/// The GCC of the heavier-inlining pass: `$LLG_TEST_HEAVY_INLINING_GCC` (empty
/// is unset), else [`gcc_driver`]. `Ok(None)` after printing why the host has
/// no usable one; `Err` when the variable names an unusable one, since the
/// caller asked for the pass.
pub(crate) fn heavy_inlining_gcc() -> Result<Option<String>, String> {
    let requested = environment(HEAVY_INLINING_GCC_VARIABLE);
    let selected = select_heavy_inlining_gcc(requested, gcc_driver, gnu_gcc_major);
    match selected {
        Ok(gcc) => Ok(Some(gcc)),
        Err(Unusable::Requested(reason)) => Err(reason),
        Err(Unusable::Host(reason)) => {
            eprintln!("SKIP: {reason}");
            Ok(None)
        }
    }
}

/// Why [`select_heavy_inlining_gcc`] found no GCC.
#[derive(Debug, PartialEq)]
pub(crate) enum Unusable {
    Requested(String),
    Host(String),
}

/// The selection of [`heavy_inlining_gcc`], with `major` the GNU GCC major
/// version of a compiler.
pub(crate) fn select_heavy_inlining_gcc(
    requested: Option<String>,
    host: impl FnOnce() -> Option<String>,
    major: impl Fn(&str) -> Option<u32>,
) -> Result<String, Unusable> {
    let explicit = requested.is_some();
    let gcc = requested.or_else(host);
    let found = gcc.as_deref().and_then(major);
    if let (Some(gcc), Some(HEAVY_INLINING_MIN_GCC..)) = (&gcc, found) {
        return Ok(gcc.clone());
    }
    let found = found.map_or_else(|| "no GNU GCC".to_owned(), |major| format!("GCC {major}"));
    let needs = format!(
        "the heavier-inlining maybe-uninitialized pass needs GCC {HEAVY_INLINING_MIN_GCC} or newer"
    );
    Err(match gcc {
        Some(gcc) if explicit => Unusable::Requested(format!(
            "{needs}, but ${HEAVY_INLINING_GCC_VARIABLE} names `{gcc}` (found {found})"
        )),
        _ => Unusable::Host(format!(
            "{needs} (found {found}); ${HEAVY_INLINING_GCC_VARIABLE} can name one"
        )),
    })
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

/// Every MSVC `/W4 /WX` build here defines `_CRT_SECURE_NO_WARNINGS`, as the
/// `runtime_value_storage` CMake probes do: the UCRT deprecates portable C11
/// routines (`sprintf`, `fopen`, `getenv`, ...) in favour of Annex K `_s`
/// variants (C4996), which strict portable C cannot use. Narrowing and other
/// `/W4` diagnostics stay errors.
const CRT_SECURE_NO_WARNINGS: &str = "/D_CRT_SECURE_NO_WARNINGS";

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
            CRT_SECURE_NO_WARNINGS,
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

/// Both output streams of a compiler run, for failure messages. MSVC `cl`
/// writes its diagnostics to stdout and GNU-like drivers to stderr, so
/// printing only one stream hides the error from the other family.
pub(crate) fn compiler_diagnostics(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

/// GNU GCC options that make GCC 13 and newer report the
/// `-Wmaybe-uninitialized` findings GCC 11 (Rocky Linux 9, Ubuntu 22.04)
/// reports at `-O2`/`-O3`. Newer jump threading removes the path on which a
/// local filled through an inlined out-parameter looks unset; without it GCC
/// 14 reproduced CI's `llg_vpi_dynamic.c:2061: 'ai' may be used
/// uninitialized` (measured: only together with `-O3`). See
/// `docs/cross_platform.md` (Compiler versions).
pub(crate) const GCC11_MAYBE_UNINITIALIZED_FLAGS: &[&str] = &["-fno-thread-jumps"];

/// True when `compiler` is GNU GCC rather than a Clang driver installed under
/// the same name (macOS `gcc` is Apple Clang). GCC-only diagnostics such as
/// `-Werror=jump-misses-init` must be requested only from real GCC: Clang
/// reports them as unknown warning options.
pub(crate) fn is_gnu_gcc(compiler: &str) -> bool {
    gnu_gcc_major(compiler).is_some()
}

/// The major version of `compiler` when it is GNU GCC ([`is_gnu_gcc`]).
/// Probed once per name per process.
pub(crate) fn gnu_gcc_major(compiler: &str) -> Option<u32> {
    static PROBED: OnceLock<Mutex<HashMap<String, Option<u32>>>> = OnceLock::new();
    let probed = PROBED.get_or_init(Default::default);
    if let Some(&known) = probed
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(compiler)
    {
        return known;
    }
    let major = probe_gnu_gcc_major(compiler);
    probed
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(compiler.to_owned(), major);
    major
}

fn probe_gnu_gcc_major(compiler: &str) -> Option<u32> {
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
    let value = |name: &str| {
        macros.lines().find_map(|line| {
            let mut words = line.split_whitespace().skip(1);
            (words.next() == Some(name)).then(|| words.next().unwrap_or_default())
        })
    };
    if value("__clang__").is_some() {
        return None;
    }
    value("__GNUC__")?.parse().ok()
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

/// Compile `source` (with extra `flags` and `$LLG_CFLAGS`) and pack it into a
/// static archive `lib<stem>.a` (`<stem>.lib` for MSVC) in `directory`, with
/// `src/sim/rt` on the include path. Static archives are the portable route
/// for C companions that call back into the model (DPI-C exports, VPI
/// applications): the model links them, so no dynamic symbol lookup is needed
/// on any host.
pub(crate) fn static_archive(
    source: &Path,
    directory: &Path,
    stem: &str,
    flags: &[&str],
) -> Result<PathBuf, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiler = host_c_compiler();
    let msvc = is_msvc(&compiler);
    let object = directory.join(format!("{stem}{}", if msvc { ".obj" } else { ".o" }));
    let mut compile = Command::new(&compiler);
    if msvc {
        compile
            .args([
                "/nologo",
                "/std:c11",
                "/W4",
                "/WX",
                CRT_SECURE_NO_WARNINGS,
                "/c",
            ])
            .arg(format!("/I{}", root.join("src/sim/rt").display()))
            .arg(format!("/Fo{}", object.display()));
    } else {
        compile
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", "-c", "-I"])
            .arg(root.join("src/sim/rt"))
            .arg("-o")
            .arg(&object);
    }
    if let Ok(cflags) = std::env::var("LLG_CFLAGS") {
        compile.args(cflags.split_whitespace());
    }
    compile.args(flags).arg(source);
    let output = compile
        .output()
        .map_err(|error| format!("{compiler}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{}: {}{}",
            source.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let archive = directory.join(if msvc {
        format!("{stem}.lib")
    } else {
        format!("lib{stem}.a")
    });
    let mut pack = if msvc {
        let mut command = Command::new("lib");
        command
            .arg("/nologo")
            .arg(format!("/OUT:{}", archive.display()));
        command
    } else {
        let mut command = Command::new("ar");
        command.arg("rcs").arg(&archive);
        command
    };
    let output = pack
        .arg(&object)
        .output()
        .map_err(|error| format!("archiver: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{} archive: {}{}",
            stem,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(archive)
}

/// A shared DPI-C or VPI library built by [`shared_library`]: `link` is the
/// file given to `--dpi-lib` (the shared object itself, or a DLL's import
/// library on Windows) and `module` the file the loader opens
/// (`LLG_VPI_PLUGIN`).
#[derive(Clone, Debug)]
pub(crate) struct SharedLibrary {
    pub link: PathBuf,
    pub module: PathBuf,
}

/// `flag` in the spelling of the compiler family: GCC/Clang
/// `-funsigned-char` is MSVC `/J` (plain `char` unsigned).
pub(crate) fn native_flag(flag: &str, msvc: bool) -> &str {
    match flag {
        "-funsigned-char" if msvc => "/J",
        _ => flag,
    }
}

/// Build `source` (strict C11, warnings as errors, then `$LLG_CFLAGS` and the
/// extra `flags`) into a shared library named after `stem` in `directory`,
/// with `src/sim/rt` on the include path.
///
/// - GCC/Clang: `lib<stem>.so` with `-shared -fPIC`; on macOS
///   `-undefined dynamic_lookup` leaves the model's symbols to dyld.
/// - MSVC: `<stem>_dll.dll` with `/LD`, plus its import library
///   `<stem>_dll.lib` for `--dpi-lib`. A DLL resolves every symbol when it
///   is linked, so a companion that calls `svdpi.h` or VPI routines or a
///   DPI-C export links against `model_import_library`, the model
///   executable's import library (`llg --dpi-import-lib`). `/W4 /WX` is the
///   MSVC counterpart of `-Wall -Wextra -Werror`. `/MD` links the DLL to the
///   shared UCRT that CMake builds the model with: `/LD` alone implies the
///   static CRT (`/MT`), whose separate `stdout` buffer would hold the
///   companion's `printf` output until the DLL unloads, after every line
///   the model printed.
///
/// `Err` carries both output streams of a failed build.
pub(crate) fn shared_library(
    source: &Path,
    directory: &Path,
    stem: &str,
    flags: &[&str],
    model_import_library: Option<&Path>,
) -> Result<SharedLibrary, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiler = host_c_compiler();
    let msvc = is_msvc(&compiler);
    let mut command = Command::new(&compiler);
    let library = if msvc {
        let module = directory.join(format!("{stem}_dll.dll"));
        command
            .args([
                "/nologo",
                "/std:c11",
                "/W4",
                "/WX",
                CRT_SECURE_NO_WARNINGS,
                "/LD",
                "/MD",
            ])
            .arg(format!("/I{}", root.join("src/sim/rt").display()))
            .arg(format!(
                "/Fo{}",
                directory.join(format!("{stem}_dll.obj")).display()
            ))
            .arg(format!("/Fe{}", module.display()));
        SharedLibrary {
            link: directory.join(format!("{stem}_dll.lib")),
            module,
        }
    } else {
        let module = directory.join(format!("lib{stem}.so"));
        command
            .args([
                "-std=c11", "-Wall", "-Wextra", "-Werror", "-shared", "-fPIC",
            ])
            .arg("-I")
            .arg(root.join("src/sim/rt"));
        if cfg!(target_os = "macos") {
            command.args(["-undefined", "dynamic_lookup"]);
        }
        SharedLibrary {
            link: module.clone(),
            module,
        }
    };
    if let Ok(cflags) = std::env::var("LLG_CFLAGS") {
        command.args(cflags.split_whitespace());
    }
    command.args(flags.iter().map(|flag| native_flag(flag, msvc)));
    command.arg(source);
    if msvc {
        if let Some(import) = model_import_library {
            command.arg(import);
        }
    } else {
        command.arg("-o").arg(&library.module);
    }
    let output = command
        .output()
        .map_err(|error| format!("{compiler}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{}: {}",
            source.display(),
            compiler_diagnostics(&output)
        ));
    }
    Ok(library)
}
