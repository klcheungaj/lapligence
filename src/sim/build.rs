//! build — the CMake model builder (the only supported model-build path).
//!
//! [`build_model_cmake`] (or [`build_model_cmake_with_opts`]) writes the
//! generated sources into `out_dir` (shared helper [`super::write_sim_sources`]),
//! emits a `CMakeLists.txt`, then runs
//! `cmake -S <out_dir> -B <out_dir>/build ... && cmake --build
//! <out_dir>/build --config Release`.  [`generate_model_sources`] performs
//! only the first half (`llg --gen-only`).  This module owns the
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
//! none (cmake picks its default generator for the host). The optional
//! [`CmakeBuildOpts::launcher`] is forwarded without selecting a default.
//!
//! Normal builds compile the runtime into the repository-local
//! `target/llg-runtime-cache` (override with `LLG_RUNTIME_CACHE_DIR`) and
//! link each generated model against the cached static archive. The cache key
//! covers the packed-value ABI, sources, toolchain, flags, generator, launcher,
//! platform, and waveform support. `--gen-only` output remains self-contained.
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
//! - `LLG_RUNTIME_CACHE_DIR` — shared static-runtime cache root; defaults to
//!   `target/llg-runtime-cache` under the Cargo workspace. Relative overrides
//!   are resolved from the workspace root.

use std::error::Error;
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// The generated project file. Cached builds compile only the model sources;
/// self-contained `--gen-only` output retains the runtime and libaco sources.
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
if(LLG_RUNTIME_LIBRARY)
  add_library(llg_runtime STATIC IMPORTED)
  set_target_properties(llg_runtime PROPERTIES IMPORTED_LOCATION "${LLG_RUNTIME_LIBRARY}")
  add_executable(sim {MODEL_SOURCES})
  target_link_libraries(sim PRIVATE llg_runtime)
else()
  add_executable(sim {ALL_SOURCES})
endif()
target_compile_definitions(sim PRIVATE LLG_MODEL_MAX_WIDTH={MODEL_WIDTH})
set_target_properties(sim PROPERTIES ENABLE_EXPORTS ON)
if(NOT MSVC)
  target_link_libraries(sim PRIVATE m)
  if(UNIX AND NOT APPLE)
    target_link_libraries(sim PRIVATE dl)
  endif()
endif()
{WAVE_SETUP}
{DPI_LINK}
"#;

const RUNTIME_CMAKELISTS_TEMPLATE: &str = r#"cmake_minimum_required(VERSION 3.16)
project(llg_sim_runtime C ASM)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
add_library(llg_runtime STATIC {RUNTIME_SOURCES})
target_include_directories(llg_runtime PRIVATE ${CMAKE_SOURCE_DIR})
target_compile_definitions(llg_runtime PRIVATE LLG_MODEL_MAX_WIDTH={MODEL_WIDTH})
{WAVE_DEFINITION}
"#;

const WAVE_CMAKE: &str = r#"find_package(Threads REQUIRED)
find_package(ZLIB REQUIRED)
target_link_libraries(sim PRIVATE Threads::Threads ZLIB::ZLIB)
target_compile_definitions(sim PRIVATE LLG_WAVEFORM=1 FST_CONFIG_INCLUDE=\"fst_config.h\")"#;
const RUNTIME_WAVE_DEFINITION: &str =
    "target_compile_definitions(llg_runtime PRIVATE LLG_WAVEFORM=1 FST_CONFIG_INCLUDE=\\\"fst_config.h\\\")";

const RUNTIME_CACHE_LOCK_TIMEOUT: Duration = Duration::from_secs(300);
const RUNTIME_CACHE_LOCK_POLL: Duration = Duration::from_millis(25);

/// Options for [`build_model_cmake_with_opts`].
#[derive(Default, Clone)]
pub struct CmakeBuildOpts {
    /// Explicit cmake `-G` generator backend (e.g. `"Ninja"`,
    /// `"Unix Makefiles"`).  Takes precedence over `$CMAKE_GENERATOR`; when
    /// `None`, `$CMAKE_GENERATOR` is forwarded if set and otherwise cmake
    /// chooses its host default.
    pub generator: Option<String>,
    /// Explicit user DPI-C libraries. Paths are passed to CMake as link
    /// items, so missing or non-files are rejected before configuration and
    /// no ambient linker search path can silently select a different ABI.
    pub dpi_libraries: Vec<PathBuf>,
    /// Optional C compiler launcher handed to CMake as
    /// `CMAKE_C_COMPILER_LAUNCHER` (for example `ccache` or `sccache`). No
    /// launcher is selected when this is `None`.
    pub launcher: Option<String>,
}

/// Failure while writing, configuring, or compiling a generated model.
#[derive(Debug)]
#[non_exhaustive]
pub enum BuildError {
    /// A direct filesystem operation in this module failed.
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    /// One `LLG_CFLAGS` token cannot be represented safely in CMake's cache.
    InvalidCompilerFlag(String),
    /// Generated source requested an invalid packed-value ABI capacity.
    InvalidModelWidth(String),
    /// Generated source requested invalid coroutine stack metadata.
    InvalidModelStack(String),
    /// A user-supplied DPI-C library is missing or cannot be represented
    /// safely in the generated CMake file.
    InvalidDpiLibrary { path: PathBuf, reason: String },
    /// The configured CMake program could not be launched.
    CmakeLaunch { program: String, source: io::Error },
    /// CMake configuration failed after one clean retry.
    Configure { command: String, output: String },
    /// Compilation of the generated C project failed.
    Compile { output: String },
    /// CMake succeeded but no simulator executable was produced.
    ExecutableNotFound { directory: PathBuf, listing: String },
    /// CMake succeeded but no cached runtime archive was produced.
    RuntimeLibraryNotFound { directory: PathBuf },
    /// Another process did not finish populating the shared runtime cache.
    RuntimeCacheLock { directory: PathBuf },
}

impl BuildError {
    /// Compatibility helper for callers that previously searched a string
    /// error. Prefer matching variants for recovery decisions.
    pub fn contains(&self, pattern: &str) -> bool {
        self.to_string().contains(pattern)
    }
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                action,
                path,
                source,
            } => write!(f, "{action} {}: {source}", path.display()),
            Self::InvalidCompilerFlag(flag) => write!(
                f,
                "LLG_CFLAGS flag `{flag}` contains a double quote; quoted flags cannot be passed through the CMake cache"
            ),
            Self::InvalidModelWidth(width) => write!(f, "invalid generated model packed width `{width}`; expected 1..{}", super::emit_c::LLG_WIDTH_LIMIT),
            Self::InvalidModelStack(value) => write!(f, "invalid generated model stack value count `{value}`"),
            Self::InvalidDpiLibrary { path, reason } => write!(
                f,
                "invalid DPI-C library {}: {reason}",
                path.display()
            ),
            Self::CmakeLaunch { program, source } => write!(
                f,
                "cmake not found or not runnable: {program} (install cmake): {source}"
            ),
            Self::Configure { command, output } => {
                write!(f, "cmake configure failed ({command}):\n{output}")
            }
            Self::Compile { output } => write!(f, "cmake build failed:\n{output}"),
            Self::ExecutableNotFound { directory, listing } => write!(
                f,
                "sim executable not found under {}: {listing}",
                directory.display()
            ),
            Self::RuntimeLibraryNotFound { directory } => write!(
                f,
                "shared runtime archive not found under {}",
                directory.display()
            ),
            Self::RuntimeCacheLock { directory } => write!(
                f,
                "timed out waiting for shared runtime cache {}",
                directory.display()
            ),
        }
    }
}

impl Error for BuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } | Self::CmakeLaunch { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Build the simulation model in `out_dir` with CMake (default options) and
/// return the path of the resulting executable.  Sources are written exactly
/// like [`generate_model_sources`] does (runtime + libaco + `extra`).
pub fn build_model_cmake(out_dir: &Path, extra: &[(&str, &str)]) -> Result<PathBuf, BuildError> {
    build_model_cmake_with_opts(out_dir, extra, &CmakeBuildOpts::default())
}

/// Build the simulation model in `out_dir` with CMake and per-call options;
/// returns the path of the resulting executable (`<out_dir>/build/bin/sim`).
pub fn build_model_cmake_with_opts(
    out_dir: &Path,
    extra: &[(&str, &str)],
    opts: &CmakeBuildOpts,
) -> Result<PathBuf, BuildError> {
    validate_dpi_libraries(opts)?;
    generate_model_sources_with_opts(out_dir, extra, opts)?;

    let cc = resolve_cc();
    let flags = c_flags()?;
    let build_dir = out_dir.join("build");
    let cmake_prog = resolve_cmake();
    let width = model_capacity(extra)?;
    let waveform = waveform_enabled(extra);
    let runtime_library = prepare_runtime_cache(width, waveform, &cc, &flags, &cmake_prog, opts)?;

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
    configure.arg(format!(
        "-DCMAKE_C_COMPILER_LAUNCHER={}",
        opts.launcher.as_deref().unwrap_or("")
    ));
    configure
        .arg(format!("-DCMAKE_C_COMPILER={cc}"))
        .arg(format!("-DCMAKE_C_FLAGS:STRING={flags}"))
        .arg(format!(
            "-DLLG_RUNTIME_LIBRARY={}",
            runtime_library.display()
        ));
    let launch_error = |source| BuildError::CmakeLaunch {
        program: cmake_prog.clone(),
        source,
    };
    let mut output = configure.output().map_err(&launch_error)?;
    if !output.status.success() {
        remove_dir_all_quiet(&build_dir);
        output = configure.output().map_err(&launch_error)?;
    }
    if !output.status.success() {
        return Err(BuildError::Configure {
            command: format!("{configure:?}"),
            output: output_tail(&output),
        });
    }

    // Build.
    let mut build_cmd = Command::new(&cmake_prog);
    build_cmd
        .arg("--build")
        .arg(&build_dir)
        .arg("--config")
        .arg("Release");
    let output = build_cmd.output().map_err(launch_error)?;
    if !output.status.success() {
        return Err(BuildError::Compile {
            output: output_tail(&output),
        });
    }

    find_sim_exe(&build_dir.join("bin"))
}

/// Write the runtime + libaco sources plus `extra` (the generated `model.c`)
/// and the generated `CMakeLists.txt` into `out_dir` — everything
/// [`build_model_cmake_with_opts`] needs except actually invoking cmake.
/// Used by `llg --gen-only`.
///
/// The directory is left deterministic: after writing, entries that are not
/// part of the current source set (and not the CMake `build/` directory) are
/// deleted, so artifacts of earlier runs never accumulate.
pub fn generate_model_sources(out_dir: &Path, extra: &[(&str, &str)]) -> Result<(), BuildError> {
    generate_model_sources_with_opts(out_dir, extra, &CmakeBuildOpts::default())
}

/// Write generated sources and CMake metadata with explicit build options.
/// This is also used by `llg --gen-only`, so the printed project remains
/// buildable with the same user DPI libraries supplied to the driver.
pub fn generate_model_sources_with_opts(
    out_dir: &Path,
    extra: &[(&str, &str)],
    opts: &CmakeBuildOpts,
) -> Result<(), BuildError> {
    validate_dpi_libraries(opts)?;
    super::write_sim_sources(out_dir, extra)?;
    let waveform = waveform_enabled(extra);
    if waveform {
        super::rt::write_waveform_sources(out_dir)?;
    }
    write_cmakelists(out_dir, extra, waveform, opts)?;
    prune_stale_entries(out_dir, extra, waveform);
    Ok(())
}

/// File names [`super::write_sim_sources`] always writes (must mirror its
/// fixed list there) plus this module's own `CMakeLists.txt`.
const FIXED_SOURCE_NAMES: [&str; 21] = [
    "llg_rt.h",
    "llg_rt.c",
    "llg_value.h",
    "llg_value.c",
    "llg_random.h",
    "llg_random.c",
    "llg_rng.h",
    "llg_rng.c",
    "vpi_user.h",
    "llg_vpi.h",
    "llg_vpi.c",
    "llg_container.h",
    "llg_container.c",
    "llg_string.h",
    "llg_string.c",
    "aco.h",
    "aco.c",
    "acosw.S",
    "aco_assert_override.h",
    "svdpi.h",
    "CMakeLists.txt",
];

/// Delete every direct child of `out_dir` that is neither a current source
/// file nor the `build/` directory (rm-all-then-write semantics for the flat
/// source set).  Refuses to run unless the canonicalized `out_dir` is an
/// absolute path at least two levels below the filesystem root: unresolvable
/// (e.g. empty) paths, `/`, and shallow roots like `/tmp` are rejected, while
/// real callers (`target/sim/<design>`, tempdir subdirectories) are
/// unaffected.
fn prune_stale_entries(out_dir: &Path, extra: &[(&str, &str)], waveform: bool) {
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
    if waveform {
        expected.extend(super::rt::waveform_sources().iter().map(|(name, _)| *name));
    }
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
fn write_cmakelists(
    out_dir: &Path,
    extra: &[(&str, &str)],
    waveform: bool,
    opts: &CmakeBuildOpts,
) -> Result<(), BuildError> {
    // Keep validating emitted frame metadata even though it is now consumed
    // only by model.c at runtime initialization, not by runtime compilation.
    let _ = model_stack_values(extra)?;
    let mut sources: Vec<&str> = extra
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| name.ends_with(".c") || name.ends_with(".S"))
        .collect();
    sources.extend([
        "llg_value.c",
        "llg_rng.c",
        "llg_rt.c",
        "llg_random.c",
        "llg_vpi.c",
        "aco.c",
        "acosw.S",
        "llg_container.c",
        "llg_string.c",
    ]);
    if waveform {
        sources.extend(["llg_wave.c", "fstapi.c", "fastlz.c", "lz4.c"]);
    }
    let model_sources: Vec<&str> = extra
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| name.ends_with(".c") || name.ends_with(".S"))
        .collect();
    let cmakelists = CMAKELISTS_TEMPLATE
        .replace("{MODEL_SOURCES}", &model_sources.join(" "))
        .replace("{ALL_SOURCES}", &sources.join(" "))
        .replace("{MODEL_WIDTH}", &model_capacity(extra)?.to_string())
        .replace("{WAVE_SETUP}", if waveform { WAVE_CMAKE } else { "" })
        .replace("{DPI_LINK}", &dpi_link_setup(opts)?);
    let cmakelists_path = out_dir.join("CMakeLists.txt");
    std::fs::write(&cmakelists_path, cmakelists).map_err(|source| BuildError::Io {
        action: "write",
        path: cmakelists_path,
        source,
    })
}

fn validate_dpi_libraries(opts: &CmakeBuildOpts) -> Result<(), BuildError> {
    for path in &opts.dpi_libraries {
        canonical_dpi_library(path)?;
    }
    Ok(())
}

/// Validate one library and return an absolute spelling for CMake. Relative
/// command-line paths are resolved against the driver's CWD, not the generated
/// model directory, so emitting them unchanged would link a different path.
fn canonical_dpi_library(path: &Path) -> Result<PathBuf, BuildError> {
    if !path.is_file() {
        return Err(BuildError::InvalidDpiLibrary {
            path: path.to_path_buf(),
            reason: "path is not a regular file".to_owned(),
        });
    }
    let canonical = path
        .canonicalize()
        .map_err(|error| BuildError::InvalidDpiLibrary {
            path: path.to_path_buf(),
            reason: format!("cannot resolve path: {error}"),
        })?;
    let Some(path_text) = canonical.to_str() else {
        return Err(BuildError::InvalidDpiLibrary {
            path: path.to_path_buf(),
            reason: "path is not valid UTF-8".to_owned(),
        });
    };
    if path_text.contains('"') {
        return Err(BuildError::InvalidDpiLibrary {
            path: path.to_path_buf(),
            reason: "path contains a double quote".to_owned(),
        });
    }
    if path_text.contains('$') || path_text.contains('\n') || path_text.contains('\r') {
        return Err(BuildError::InvalidDpiLibrary {
            path: path.to_path_buf(),
            reason: "path contains a CMake interpolation or line-break character".to_owned(),
        });
    }
    Ok(canonical)
}

fn dpi_link_setup(opts: &CmakeBuildOpts) -> Result<String, BuildError> {
    if opts.dpi_libraries.is_empty() {
        return Ok(String::new());
    }
    let libraries = opts
        .dpi_libraries
        .iter()
        .map(|path| canonical_dpi_library(path))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|path| {
            // CMake accepts forward slashes on all supported hosts. Escape a
            // list separator so a Windows path cannot be split into two link
            // items when the cache is parsed.
            path.to_string_lossy()
                .replace('\\', "/")
                .replace(';', "\\;")
        })
        .map(|path| format!("\"{path}\""))
        .collect::<Vec<_>>()
        .join(" ");
    Ok(format!("target_link_libraries(sim PRIVATE {libraries})"))
}

/// Build or reuse the immutable runtime archive for one value-layout ABI.
///
/// The packed width remains part of `sv4_t`'s C layout, so it is necessarily
/// part of the cache key. Stack headroom is deliberately absent: generated
/// model code passes that value to `llg_rt_init_with_stack` at runtime.
fn prepare_runtime_cache(
    width: u32,
    waveform: bool,
    cc: &str,
    flags: &str,
    cmake_prog: &str,
    opts: &CmakeBuildOpts,
) -> Result<PathBuf, BuildError> {
    let key = runtime_cache_key(width, waveform, cc, flags, cmake_prog, opts);
    let cache_root = runtime_cache_root();
    let entry = cache_root.join(key);
    if let Some(library) = cached_runtime_library(&entry) {
        return Ok(library);
    }

    std::fs::create_dir_all(&cache_root).map_err(|source| BuildError::Io {
        action: "create runtime cache",
        path: cache_root.clone(),
        source,
    })?;
    let _lock = RuntimeCacheLock::acquire(&entry)?;
    if let Some(library) = cached_runtime_library(&entry) {
        return Ok(library);
    }

    std::fs::create_dir_all(&entry).map_err(|source| BuildError::Io {
        action: "create runtime cache entry",
        path: entry.clone(),
        source,
    })?;
    let ready_path = entry.join("ready");
    match std::fs::remove_file(&ready_path) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(source) => {
            return Err(BuildError::Io {
                action: "clear incomplete runtime cache marker",
                path: ready_path,
                source,
            });
        }
    }
    super::write_sim_sources(&entry, &[])?;
    if waveform {
        super::rt::write_waveform_sources(&entry)?;
    }
    let runtime_sources = runtime_source_names(waveform).join(" ");
    let cmakelists = RUNTIME_CMAKELISTS_TEMPLATE
        .replace("{RUNTIME_SOURCES}", &runtime_sources)
        .replace("{MODEL_WIDTH}", &width.to_string())
        .replace(
            "{WAVE_DEFINITION}",
            if waveform {
                RUNTIME_WAVE_DEFINITION
            } else {
                ""
            },
        );
    let cmakelists_path = entry.join("CMakeLists.txt");
    std::fs::write(&cmakelists_path, cmakelists).map_err(|source| BuildError::Io {
        action: "write runtime cache project",
        path: cmakelists_path,
        source,
    })?;

    let build_dir = entry.join("build");
    if build_dir.exists() {
        match cached_generator(&build_dir) {
            None => remove_dir_all_quiet(&build_dir),
            Some(cached) => {
                if generator_for(opts).is_some_and(|requested| requested != cached) {
                    remove_dir_all_quiet(&build_dir);
                }
            }
        }
    }

    let mut configure = Command::new(cmake_prog);
    configure.arg("-S").arg(&entry).arg("-B").arg(&build_dir);
    if let Some(generator) = generator_for(opts) {
        configure.arg("-G").arg(generator);
    }
    configure
        .arg(format!("-DCMAKE_C_COMPILER={cc}"))
        .arg(format!("-DCMAKE_C_FLAGS:STRING={flags}"))
        .arg(format!(
            "-DCMAKE_C_COMPILER_LAUNCHER={}",
            opts.launcher.as_deref().unwrap_or("")
        ));
    let launch_error = |source| BuildError::CmakeLaunch {
        program: cmake_prog.to_owned(),
        source,
    };
    let mut output = configure.output().map_err(&launch_error)?;
    if !output.status.success() {
        remove_dir_all_quiet(&build_dir);
        output = configure.output().map_err(&launch_error)?;
    }
    if !output.status.success() {
        return Err(BuildError::Configure {
            command: format!("{configure:?}"),
            output: output_tail(&output),
        });
    }

    let mut build = Command::new(cmake_prog);
    build
        .arg("--build")
        .arg(&build_dir)
        .arg("--config")
        .arg("Release")
        .arg("--target")
        .arg("llg_runtime");
    let output = build.output().map_err(launch_error)?;
    if !output.status.success() {
        return Err(BuildError::Compile {
            output: output_tail(&output),
        });
    }
    let library =
        find_runtime_library(&build_dir).ok_or_else(|| BuildError::RuntimeLibraryNotFound {
            directory: build_dir.clone(),
        })?;
    std::fs::write(&ready_path, b"ready\n").map_err(|source| BuildError::Io {
        action: "mark runtime cache ready",
        path: ready_path,
        source,
    })?;
    Ok(library)
}

fn runtime_source_names(waveform: bool) -> Vec<&'static str> {
    let mut sources = vec![
        "llg_value.c",
        "llg_rng.c",
        "llg_rt.c",
        "llg_random.c",
        "llg_vpi.c",
        "aco.c",
        "acosw.S",
        "llg_container.c",
        "llg_string.c",
    ];
    if waveform {
        sources.extend(["llg_wave.c", "fstapi.c", "fastlz.c", "lz4.c"]);
    }
    sources
}

fn runtime_cache_root() -> PathBuf {
    runtime_cache_root_with_override(std::env::var_os("LLG_RUNTIME_CACHE_DIR").map(PathBuf::from))
}

fn runtime_cache_root_with_override(override_root: Option<PathBuf>) -> PathBuf {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    match override_root {
        Some(path) if path.is_absolute() => path,
        Some(path) => workspace.join(path),
        None => workspace.join("target/llg-runtime-cache"),
    }
}

fn runtime_cache_key(
    width: u32,
    waveform: bool,
    cc: &str,
    flags: &str,
    cmake_prog: &str,
    opts: &CmakeBuildOpts,
) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    let compiler = compiler_identity(cc);
    let generator = generator_for(opts).unwrap_or_default();
    for text in [
        RUNTIME_CMAKELISTS_TEMPLATE,
        RUNTIME_WAVE_DEFINITION,
        super::rt::runtime_sources().0,
        super::rt::runtime_sources().1,
        super::rt::value_sources().0,
        super::rt::value_sources().1,
        super::rt::random_sources().0,
        super::rt::random_sources().1,
        super::rt::rng_sources().0,
        super::rt::rng_sources().1,
        super::rt::vpi_sources().0,
        super::rt::vpi_sources().1,
        super::rt::vpi_bridge_header(),
        super::rt::container_sources().0,
        super::rt::container_sources().1,
        super::rt::string_sources().0,
        super::rt::string_sources().1,
        super::rt::libaco_sources().0,
        super::rt::libaco_sources().1,
        super::rt::libaco_sources().2,
        include_str!("../../vendor/libaco/aco_assert_override.h"),
        include_str!("../../vendor/slang/external/ieee1800/svdpi.h"),
        cc,
        flags,
        cmake_prog,
        &compiler,
        &generator,
        opts.launcher.as_deref().unwrap_or(""),
        std::env::consts::OS,
        std::env::consts::ARCH,
    ] {
        for byte in text.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    if waveform {
        for (_, source) in super::rt::waveform_sources() {
            for byte in source.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
    }
    format!("w{width}-wave{}-{hash:016x}", u8::from(waveform))
}

fn compiler_identity(cc: &str) -> String {
    let mut identity = cc.to_owned();
    for argument in ["--version", "-dumpmachine"] {
        if let Ok(output) = Command::new(cc).arg(argument).output() {
            identity.push('\n');
            identity.push_str(&String::from_utf8_lossy(&output.stdout));
            identity.push_str(&String::from_utf8_lossy(&output.stderr));
        }
    }
    identity
}

fn find_runtime_library(build_dir: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    collect_named_files(build_dir, "libllg_runtime.a", &mut found);
    collect_named_files(build_dir, "llg_runtime.lib", &mut found);
    found.sort();
    found.into_iter().next()
}

fn cached_runtime_library(entry: &Path) -> Option<PathBuf> {
    entry
        .join("ready")
        .is_file()
        .then(|| find_runtime_library(&entry.join("build")))
        .flatten()
}

/// Process-owned lock for one cache key.
///
/// The file stays in the cache so waiters always open the same inode. The OS
/// releases the lock when its process exits, including abrupt test shutdowns.
struct RuntimeCacheLock {
    _file: File,
}

impl RuntimeCacheLock {
    fn acquire(entry: &Path) -> Result<Self, BuildError> {
        let path = entry.with_extension("lock");
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|source| BuildError::Io {
                action: "open runtime cache lock",
                path: path.clone(),
                source,
            })?;
        let started = Instant::now();
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Self { _file: file }),
                Err(TryLockError::WouldBlock) => {
                    if started.elapsed() > RUNTIME_CACHE_LOCK_TIMEOUT {
                        return Err(BuildError::RuntimeCacheLock {
                            directory: path.clone(),
                        });
                    }
                    std::thread::sleep(RUNTIME_CACHE_LOCK_POLL);
                }
                Err(TryLockError::Error(source)) => {
                    return Err(BuildError::Io {
                        action: "lock runtime cache",
                        path: path.clone(),
                        source,
                    });
                }
            }
        }
    }
}

fn model_capacity(extra: &[(&str, &str)]) -> Result<u32, BuildError> {
    let mut capacity = None;
    for (_, source) in extra {
        for line in source.lines() {
            if let Some(value) = line.strip_prefix("#define LLG_MODEL_MAX_WIDTH ") {
                let width = value
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|width| (1..super::emit_c::LLG_WIDTH_LIMIT).contains(width))
                    .ok_or_else(|| BuildError::InvalidModelWidth(value.to_string()))?;
                if capacity.is_some_and(|previous| previous != width) {
                    return Err(BuildError::InvalidModelWidth(format!(
                        "conflicting capacities {capacity:?} and {width}"
                    )));
                }
                capacity = Some(width);
            }
        }
    }
    // Standalone C runtime self-tests do not carry generated model metadata.
    Ok(capacity.unwrap_or(1024))
}

fn model_stack_values(extra: &[(&str, &str)]) -> Result<u64, BuildError> {
    let mut slots = None;
    for (_, source) in extra {
        for line in source.lines() {
            if let Some(value) = line.strip_prefix("#define LLG_MODEL_STACK_VALUES ") {
                let parsed = value
                    .trim()
                    .parse::<u64>()
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| BuildError::InvalidModelStack(value.to_string()))?;
                if slots.is_some_and(|previous| previous != parsed) {
                    return Err(BuildError::InvalidModelStack(
                        "conflicting stack counts".into(),
                    ));
                }
                slots = Some(parsed);
            }
        }
    }
    Ok(slots.unwrap_or(256))
}

fn waveform_enabled(extra: &[(&str, &str)]) -> bool {
    extra.iter().any(|(_, text)| {
        text.lines()
            .any(|line| line.trim_end() == "#define LLG_WAVEFORM 1")
    })
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
fn c_flags() -> Result<String, BuildError> {
    let mut flags = String::from("-O2 -Wall -Wno-unused-function");
    if let Ok(extra) = std::env::var("LLG_CFLAGS") {
        for flag in extra.split_whitespace() {
            if flag.contains('"') {
                return Err(BuildError::InvalidCompilerFlag(flag.to_string()));
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
fn find_sim_exe(bin_dir: &Path) -> Result<PathBuf, BuildError> {
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
    Err(BuildError::ExecutableNotFound {
        directory: bin_dir.to_path_buf(),
        listing: listed,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn model_metadata_selects_one_consistent_runtime_abi() {
        assert_eq!(model_capacity(&[]).unwrap(), 1024);
        assert_eq!(model_stack_values(&[]).unwrap(), 256);
        let source = "#define LLG_MODEL_MAX_WIDTH 65536\n#define LLG_MODEL_STACK_VALUES 4096\n";
        assert_eq!(model_capacity(&[("model.c", source)]).unwrap(), 65536);
        assert_eq!(model_stack_values(&[("model.c", source)]).unwrap(), 4096);
        for invalid in ["0", "1048576", "1048577", "4294967296", "(1 << 20)"] {
            let source = format!("#define LLG_MODEL_MAX_WIDTH {invalid}\n");
            assert!(matches!(
                model_capacity(&[("model.c", &source)]),
                Err(BuildError::InvalidModelWidth(_))
            ));
        }
        assert!(model_capacity(&[
            ("a.c", "#define LLG_MODEL_MAX_WIDTH 128\n"),
            ("b.c", "#define LLG_MODEL_MAX_WIDTH 256\n")
        ])
        .is_err());
        assert!(model_stack_values(&[("a.c", "#define LLG_MODEL_STACK_VALUES 0\n")]).is_err());
    }

    #[test]
    fn runtime_cache_key_varies_with_abi_and_toolchain_options() {
        let defaults = CmakeBuildOpts::default();
        let base = runtime_cache_key(64, false, "cc", "-O2", "cmake", &defaults);
        assert_eq!(
            base,
            runtime_cache_key(64, false, "cc", "-O2", "cmake", &defaults)
        );
        assert_ne!(
            base,
            runtime_cache_key(128, false, "cc", "-O2", "cmake", &defaults)
        );
        assert_ne!(
            base,
            runtime_cache_key(64, true, "cc", "-O2", "cmake", &defaults)
        );
        let launched = CmakeBuildOpts {
            launcher: Some("ccache".to_owned()),
            ..Default::default()
        };
        assert_ne!(
            base,
            runtime_cache_key(64, false, "cc", "-O2", "cmake", &launched)
        );
    }

    #[test]
    fn runtime_cache_lock_file_can_be_reused_after_release() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "llg-runtime-cache-lock-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let entry = dir.join("entry");

        let first = RuntimeCacheLock::acquire(&entry).unwrap();
        let lock_path = entry.with_extension("lock");
        assert!(lock_path.is_file());
        drop(first);

        let second = RuntimeCacheLock::acquire(&entry).unwrap();
        drop(second);
        assert!(lock_path.is_file());

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn runtime_cache_defaults_to_workspace_target_and_accepts_override() {
        let workspace_default =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/llg-runtime-cache");
        assert_eq!(runtime_cache_root_with_override(None), workspace_default);

        let relative_override = PathBuf::from("custom/runtime-cache");
        assert_eq!(
            runtime_cache_root_with_override(Some(relative_override.clone())),
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_override)
        );

        let absolute_override = std::env::temp_dir().join("llg-custom-runtime-cache");
        assert_eq!(
            runtime_cache_root_with_override(Some(absolute_override.clone())),
            absolute_override
        );
    }

    #[test]
    fn waveform_runtime_selftest() {
        if !cmake_available() {
            eprintln!("SKIP: cmake not available");
            return;
        }
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("llg-wave-selftest-{}-{unique}", std::process::id()));
        let result = (|| {
            let exe = build_model_cmake(
                &dir,
                &[(
                    "llg_wave_selftest.c",
                    super::super::rt::waveform_selftest_source(),
                )],
            )
            .map_err(|error| error.to_string())?;
            let output = Command::new(&exe)
                .current_dir(&dir)
                .output()
                .map_err(|e| format!("run {}: {e}", exe.display()))?;
            if !output.status.success() {
                return Err(format!(
                    "waveform selftest failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            generate_model_sources(&dir, &[("plain.c", "int main(void) { return 0; }\n")])
                .map_err(|error| error.to_string())?;
            if dir.join("llg_wave.c").exists() {
                return Err("ordinary source generation retained waveform files".to_string());
            }
            let cmake = std::fs::read_to_string(dir.join("CMakeLists.txt"))
                .map_err(|e| format!("read generated CMakeLists.txt: {e}"))?;
            if cmake.contains("find_package(Threads") || cmake.contains("find_package(ZLIB") {
                return Err("ordinary source generation retained waveform dependencies".to_string());
            }
            Ok::<_, String>(())
        })();
        let _ = std::fs::remove_dir_all(&dir);
        if let Err(error) = result {
            panic!("{error}");
        }
    }
}
