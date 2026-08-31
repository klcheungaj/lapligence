//! build — the CMake model builder (the only supported model-build path).
//!
//! [`build_model_cmake`] (or [`build_model_cmake_with_opts`]) writes the
//! generated sources into `out_dir` (shared helper [`super::write_sim_sources`]),
//! emits a `CMakeLists.txt`, then runs
//! `cmake -S <out_dir> -B <out_dir>/build ... && cmake --build
//! <out_dir>/build --config Release`.  [`generate_model_sources`] performs
//! only the first half (`llg_sim --gen-only`).  This module owns the
//! generated `CMakeLists.txt` and the cmake invocation.
//!
//! Rebuilds are deterministic (no disk accumulation in the output tree):
//!
//! - After writing, every entry directly under `out_dir` that is not part of
//!   the current source set (and is not the CMake `build/` directory) is
//!   deleted — stale sources or artifacts from earlier runs never linger.
//! - The CMake build dir is reused incrementally only when it is compatible:
//!   a `build/` without a readable `CMakeCache.txt` (partial/failed
//!   configure), or one whose cached `CMAKE_GENERATOR` differs from the
//!   requested `-G`, is removed entirely so cmake reconfigures from scratch.
//!   A configure that still fails (any poisoned-cache cause) gets exactly one
//!   clean-from-scratch retry before the error is reported.
//!
//! Generator selection: [`CmakeBuildOpts::generator`] > `$CMAKE_GENERATOR` >
//! none (cmake picks its default generator for the host).  The driver's
//! `--generator <backend>` flag feeds [`CmakeBuildOpts::generator`].
//!
//! Environment variables:
//!
//! - `LLG_CC` / `CC` — C compiler handed to CMake as `-DCMAKE_C_COMPILER`;
//!   falls back to `cc`.
//! - `LLG_CFLAGS` — extra whitespace-separated compiler flags appended to
//!   `-DCMAKE_C_FLAGS` (e.g. sanitizer flags).  Flags containing a double
//!   quote are rejected: they cannot be passed through the CMake cache
//!   reliably.
//! - `LLG_CMAKE` — explicit cmake program override; default `cmake`
//!   (also used by [`cmake_available`]).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// The generated project file.  `{SOURCES}` is replaced with the actual
/// source list (`model.c llg_rt.c aco.c acosw.S`); everything else is fixed.
/// ASM is enabled because libaco's context switch lives in `acosw.S`.
const CMAKELISTS_TEMPLATE: &str = r#"cmake_minimum_required(VERSION 3.16)
project(llg_sim_model C ASM)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
include_directories(${CMAKE_SOURCE_DIR})
add_executable(sim {SOURCES})
target_link_libraries(sim m)
"#;

/// Options for [`build_model_cmake_with_opts`].
#[derive(Default, Clone)]
pub struct CmakeBuildOpts {
    /// Explicit cmake `-G` generator backend (e.g. `"Ninja"`,
    /// `"Unix Makefiles"`).  Takes precedence over `$CMAKE_GENERATOR`; when
    /// `None`, `$CMAKE_GENERATOR` is forwarded if set and otherwise cmake
    /// chooses its host default.
    pub generator: Option<String>,
}

/// Build the simulation model in `out_dir` with CMake (default options) and
/// return the path of the resulting executable.  Sources are written exactly
/// like [`generate_model_sources`] does (runtime + libaco + `extra`).
pub fn build_model_cmake(out_dir: &Path, extra: &[(&str, &str)]) -> Result<PathBuf, String> {
    build_model_cmake_with_opts(out_dir, extra, &CmakeBuildOpts::default())
}

/// Build the simulation model in `out_dir` with CMake and per-call options;
/// returns the path of the resulting executable (`<out_dir>/build/bin/sim`).
pub fn build_model_cmake_with_opts(
    out_dir: &Path,
    extra: &[(&str, &str)],
    opts: &CmakeBuildOpts,
) -> Result<PathBuf, String> {
    generate_model_sources(out_dir, extra)?;

    let cc = resolve_cc();
    let flags = c_flags()?;
    let build_dir = out_dir.join("build");
    let cmake_prog = resolve_cmake();

    // Drop an incompatible CMake build tree so configure starts clean: no
    // cache means a previous configure died midway; a generator mismatch
    // would make cmake refuse the directory outright.  A compatible tree is
    // kept — reconfigure + build are incremental.
    if build_dir.exists() {
        match cached_generator(&build_dir) {
            None => remove_dir_all_quiet(&build_dir),
            Some(cached) => {
                if generator_for(opts).is_some_and(|req| req != cached) {
                    remove_dir_all_quiet(&build_dir);
                }
            }
        }
    }

    // Configure.  A failed configure leaves the tree unusable no matter the
    // cause (poisoned/stale cache, half-written state, changed toolchain), so
    // retry exactly once from scratch before reporting the error.
    let mut configure = Command::new(&cmake_prog);
    configure.arg("-S").arg(out_dir).arg("-B").arg(&build_dir);
    if let Some(generator) = generator_for(opts) {
        configure.arg("-G").arg(generator);
    }
    configure
        .arg(format!("-DCMAKE_C_COMPILER={cc}"))
        .arg(format!("-DCMAKE_C_FLAGS:STRING={flags}"));
    let mut output = configure.output().map_err(|e| {
        format!("cmake not found or not runnable: {cmake_prog} (install cmake): {e}")
    })?;
    if !output.status.success() {
        remove_dir_all_quiet(&build_dir);
        output = configure.output().map_err(|e| {
            format!("cmake not found or not runnable: {cmake_prog} (install cmake): {e}")
        })?;
    }
    if !output.status.success() {
        return Err(format!(
            "cmake configure failed ({configure:?}):\n{}",
            output_tail(&output)
        ));
    }

    // Build.
    let mut build_cmd = Command::new(&cmake_prog);
    build_cmd
        .arg("--build")
        .arg(&build_dir)
        .arg("--config")
        .arg("Release");
    let output = build_cmd.output().map_err(|e| {
        format!("cmake not found or not runnable: {cmake_prog} (install cmake): {e}")
    })?;
    if !output.status.success() {
        return Err(format!("cmake build failed:\n{}", output_tail(&output)));
    }

    find_sim_exe(&build_dir.join("bin"))
}

/// Write the runtime + libaco sources plus `extra` (the generated `model.c`)
/// and the generated `CMakeLists.txt` into `out_dir` — everything
/// [`build_model_cmake_with_opts`] needs except actually invoking cmake.
/// Used by `llg_sim --gen-only`.
///
/// The directory is left deterministic: after writing, entries that are not
/// part of the current source set (and not the CMake `build/` directory) are
/// deleted, so artifacts of earlier runs never accumulate.
pub fn generate_model_sources(out_dir: &Path, extra: &[(&str, &str)]) -> Result<(), String> {
    super::write_sim_sources(out_dir, extra)?;
    write_cmakelists(out_dir, extra)?;
    prune_stale_entries(out_dir, extra);
    Ok(())
}

/// File names [`super::write_sim_sources`] always writes (must mirror its
/// fixed list there) plus this module's own `CMakeLists.txt`.
const FIXED_SOURCE_NAMES: [&str; 7] = [
    "llg_rt.h",
    "llg_rt.c",
    "aco.h",
    "aco.c",
    "acosw.S",
    "aco_assert_override.h",
    "CMakeLists.txt",
];

/// Delete every direct child of `out_dir` that is neither a current source
/// file nor the `build/` directory (rm-all-then-write semantics for the flat
/// source set).  Refuses to run unless the canonicalized `out_dir` is an
/// absolute path at least two levels below the filesystem root: unresolvable
/// (e.g. empty) paths, `/`, and shallow roots like `/tmp` are rejected, while
/// real callers (`target/sim/<design>`, tempdir subdirectories) are
/// unaffected.
fn prune_stale_entries(out_dir: &Path, extra: &[(&str, &str)]) {
    let canonical = match out_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => return,
    };
    // canonicalize() already yields absolute paths; the explicit check keeps
    // the guard's intent obvious.  Depth >= 3 = <root>/<level>/<name>.
    if !canonical.is_absolute() || canonical.components().count() < 3 {
        return;
    }
    let mut expected: Vec<&str> = FIXED_SOURCE_NAMES.to_vec();
    expected.extend(extra.iter().map(|(name, _)| *name));
    let Ok(entries) = std::fs::read_dir(out_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            continue;
        };
        if name == "build" || expected.contains(&name) {
            continue;
        }
        let path = out_dir.join(name);
        if path.is_dir() {
            remove_dir_all_quiet(&path);
        } else if path.is_file() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Cached `CMAKE_GENERATOR` from `<build_dir>/CMakeCache.txt`, if readable.
fn cached_generator(build_dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(build_dir.join("CMakeCache.txt")).ok()?;
    for line in text.lines() {
        // Cache lines look like `CMAKE_GENERATOR:INTERNAL=Unix Makefiles`;
        // values may be quoted but never contain newlines.
        if let Some(rest) = line.strip_prefix("CMAKE_GENERATOR:") {
            return rest
                .split_once('=')
                .map(|(_, v)| v.trim_matches('"').to_string());
        }
    }
    None
}

fn remove_dir_all_quiet(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Emit `CMakeLists.txt` for the source set `extra` (+ runtime + libaco).
fn write_cmakelists(out_dir: &Path, extra: &[(&str, &str)]) -> Result<(), String> {
    let mut sources: Vec<&str> = extra
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| name.ends_with(".c") || name.ends_with(".S"))
        .collect();
    sources.extend(["llg_rt.c", "aco.c", "acosw.S"]);
    let cmakelists = CMAKELISTS_TEMPLATE.replace("{SOURCES}", &sources.join(" "));
    let cmakelists_path = out_dir.join("CMakeLists.txt");
    std::fs::write(&cmakelists_path, cmakelists)
        .map_err(|e| format!("write {}: {e}", cmakelists_path.display()))
}

/// Whether a usable cmake exists (`$LLG_CMAKE` or `cmake --version`).
/// Probed once per process; lets test suites skip gracefully on hosts
/// without cmake.
pub fn cmake_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new(resolve_cmake())
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// `-G` value: explicit option > `$CMAKE_GENERATOR` > none.
fn generator_for(opts: &CmakeBuildOpts) -> Option<String> {
    opts.generator
        .clone()
        .or_else(|| std::env::var("CMAKE_GENERATOR").ok())
}

/// `$LLG_CC`, else `$CC`, else `cc`.
fn resolve_cc() -> String {
    std::env::var("LLG_CC")
        .or_else(|_| std::env::var("CC"))
        .unwrap_or_else(|_| "cc".to_string())
}

/// `$LLG_CMAKE`, else `cmake`.
fn resolve_cmake() -> String {
    std::env::var("LLG_CMAKE").unwrap_or_else(|_| "cmake".to_string())
}

/// `-DCMAKE_C_FLAGS` payload: the base warning/optimization set plus every
/// whitespace-separated token of `$LLG_CFLAGS`.
fn c_flags() -> Result<String, String> {
    let mut flags = String::from("-O2 -Wall -Wno-unused-function");
    if let Ok(extra) = std::env::var("LLG_CFLAGS") {
        for flag in extra.split_whitespace() {
            if flag.contains('"') {
                return Err(format!(
                    "LLG_CFLAGS flag `{flag}` contains a double quote; \
                     quoted flags cannot be passed through the CMake cache"
                ));
            }
            flags.push(' ');
            flags.push_str(flag);
        }
    }
    Ok(flags)
}

/// Last lines of the captured tool output for error reporting: stderr when
/// present, stdout otherwise.
fn output_tail(output: &std::process::Output) -> String {
    let bytes = if output.stderr.is_empty() {
        &output.stdout
    } else {
        &output.stderr
    };
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    let tail = if lines.len() > 20 {
        &lines[lines.len() - 20..]
    } else {
        &lines[..]
    };
    let mut joined = tail.join("\n");
    if joined.chars().count() > 2000 {
        joined = joined.chars().skip(joined.chars().count() - 2000).collect();
    }
    joined
}

/// Locate the built executable: `<build>/bin/sim` first, then a recursive
/// search under `<build>/bin/` (multi-config generators may add per-config
/// subdirectories).  Deterministic order on all paths.
fn find_sim_exe(bin_dir: &Path) -> Result<PathBuf, String> {
    let direct = bin_dir.join("sim");
    if direct.is_file() {
        return Ok(direct);
    }
    let mut found = Vec::new();
    collect_named_files(bin_dir, "sim", &mut found);
    if let Some(path) = found.first() {
        return Ok(path.clone());
    }
    let mut listing = Vec::new();
    collect_paths(bin_dir, &mut listing);
    let listed = if listing.is_empty() {
        "(missing or empty)".to_string()
    } else {
        listing
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    Err(format!(
        "sim executable not found under {}: {listed}",
        bin_dir.display()
    ))
}

/// Recursively collect files named `name` under `dir`, sorted within each
/// directory so the result is deterministic.
fn collect_named_files(dir: &Path, name: &str, out: &mut Vec<PathBuf>) {
    let entries = match sorted_entries(dir) {
        Some(entries) => entries,
        None => return,
    };
    for path in entries {
        if path.is_dir() {
            collect_named_files(&path, name, out);
        } else if path.file_name().is_some_and(|n| n == name) {
            out.push(path);
        }
    }
}

/// Recursively collect every path under `dir`, sorted within each directory.
fn collect_paths(dir: &Path, out: &mut Vec<PathBuf>) {
    for path in sorted_entries(dir).unwrap_or_default() {
        if path.is_dir() {
            collect_paths(&path, out);
        }
        out.push(path);
    }
}

fn sorted_entries(dir: &Path) -> Option<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .collect();
    paths.sort();
    Some(paths)
}
