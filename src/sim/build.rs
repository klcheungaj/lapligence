//! build — the CMake model builder (the only supported model-build path).
//!
//! [`build_model_cmake`] (or [`build_model_cmake_with_opts`]) writes the
//! generated sources into `out_dir` (shared helper [`super::write_sim_sources`]),
//! emits a `CMakeLists.txt`, then runs
//! `cmake -S <out_dir> -B <out_dir>/build ... && cmake --build
//! <out_dir>/build --config Release --parallel <N>`.  [`generate_model_sources`] performs
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
//! [`DEFAULT_GENERATOR`] (`Ninja`) on every host; empty values count as
//! unset. A configure that finds no build program for the generator (Ninja
//! not installed) fails at once with [`BuildError::BuildProgramNotFound`]
//! instead of the clean retry. A tree configured with another generator is
//! rebuilt from scratch once, so changing the generator never loops. The optional
//! [`CmakeBuildOpts::launcher`] > `$LLG_C_LAUNCHER` > none is forwarded as
//! `CMAKE_C_COMPILER_LAUNCHER` without selecting a default. (The `llg` driver
//! layers `llg.toml` below the variable before it fills the option: command
//! line > `$LLG_C_LAUNCHER` > `build.launcher`.)
//!
//! Normal builds compile the runtime into a cache (see
//! [`CmakeBuildOpts::runtime_cache_dir`]) and
//! link each generated model against the cached static archive. The cache key
//! covers the packed-value ABI, sources, compiler-reported target, toolchain,
//! flags, generator, launcher, platform, and waveform support. `--gen-only`
//! output remains self-contained.
//! [`CmakeBuildOpts::model_opt_level`] selects optimization for both model and
//! runtime sources. Generated projects prefix compiler-specific level and
//! warning flags; Release adds only NDEBUG. Extra user flags follow the level
//! and can override it. The level and the CMake setup participate in cache keys.
//!
//! Environment variables (each is a fallback for the matching
//! [`CmakeBuildOpts`] field, which wins when set):
//!
//! - `LLG_CC` / `CC` — C compiler handed to CMake as `-DCMAKE_C_COMPILER`;
//!   empty values are unset. Falls back to [`DEFAULT_C_COMPILER`]: MSVC `cl`
//!   on Windows, `cc` elsewhere.
//! - `LLG_C_LAUNCHER` — C compiler launcher (for example `ccache`) handed to
//!   CMake as `-DCMAKE_C_COMPILER_LAUNCHER` for the runtime archive and the
//!   model; `LLG_CC` must stay one program, so a launcher needs its own
//!   variable. An empty value means none; the `--launcher` option wins, and an
//!   explicit empty option value suppresses the variable. It never affects the
//!   root `build.rs` Slang build (that uses `LLG_CCACHE`).
//! - `LLG_CFLAGS` — extra whitespace-separated compiler flags appended to
//!   `-DCMAKE_C_FLAGS` (e.g. sanitizer flags).  Flags containing a double
//!   quote are rejected: they cannot be passed through the CMake cache
//!   reliably.
//! - `LLG_CMAKE` — explicit cmake program override; default `cmake`
//!   (also used by [`cmake_available`]).
//! - `CMAKE_BUILD_PARALLEL_LEVEL` — positive job count for both
//!   `cmake --build` invocations when [`CmakeBuildOpts::build_jobs`] is unset
//!   (see [`resolve_build_jobs`]); otherwise the host's available parallelism.
//! - `LLG_RUNTIME_CACHE_DIR` — shared static-runtime cache root; defaults to
//!   `build/llg-runtime-cache` under the current directory. Relative values
//!   resolve from the current directory; an empty value selects the default.
//!   No path is fixed at compile time. Each runtime archive entry is a child
//!   directory with a `ready` marker. The only other children are the
//!   auxiliary directories in [`RUNTIME_CACHE_AUX_DIRS`]: [`TOOLCHAIN_SEED_DIR`]
//!   (`cmake-toolchain/`) holds the CMake toolchain-detection seeds that fresh
//!   build trees reuse (see `build/toolchain_seed.rs`), and
//!   [`COMPILER_PROBE_DIR`] (`compiler-probe/`) the MSVC compiler self-report
//!   memo (see `build/compiler_probe.rs`).
//! - `LLG_CMAKE_TOOLCHAIN_SEED` ([`TOOLCHAIN_SEED_ENV`]) — `0`, `off`,
//!   `false` or `no` makes every fresh configure run CMake's own toolchain
//!   detection.

mod compiler_probe;
mod legacy_pli;
mod toolchain_seed;
mod value;

pub use compiler_probe::MEMO_DIR as COMPILER_PROBE_DIR;
pub use toolchain_seed::{SEED_DIR as TOOLCHAIN_SEED_DIR, SEED_ENV as TOOLCHAIN_SEED_ENV};

/// The runtime cache root's children that are not runtime archive entries.
/// Every other child is one archive entry (a directory that is ready once it
/// holds a `ready` marker); code that enumerates or prunes the root skips
/// these names.
pub const RUNTIME_CACHE_AUX_DIRS: [&str; 2] = [TOOLCHAIN_SEED_DIR, COMPILER_PROBE_DIR];

use std::error::Error;
use std::fmt;
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Build type of every generated model and runtime configure (and `--config`
/// of its build).
const BUILD_TYPE: &str = "Release";

/// The generated project file. Cached builds compile only the model sources;
/// self-contained `--gen-only` output retains the runtime sources.
const CMAKELISTS_TEMPLATE: &str = r#"cmake_minimum_required(VERSION 3.16)
project(llg_sim_model C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
{OPTIMIZATION_SETUP}
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR}/bin)
include_directories(${CMAKE_SOURCE_DIR})
set_source_files_properties(llg_co.c PROPERTIES COMPILE_DEFINITIONS LLG_CO_HOST_ALLOC=1)
if(LLG_RUNTIME_LIBRARY)
  add_library(llg_runtime STATIC IMPORTED)
  set_target_properties(llg_runtime PROPERTIES IMPORTED_LOCATION "${LLG_RUNTIME_LIBRARY}")
  add_executable(sim {MODEL_SOURCES})
  target_link_libraries(sim PRIVATE llg_runtime)
else()
  add_library(llg_runtime STATIC {RUNTIME_SOURCES})
  add_executable(sim {MODEL_SOURCES})
  target_link_libraries(sim PRIVATE llg_runtime)
endif()
set_target_properties(sim PROPERTIES ENABLE_EXPORTS ON)
{MODEL_SOURCE_OPTIONS}if(MSVC)
  # The estimate matches LLG_HOST_STACK_ESTIMATE_BYTES in llg_rt.h. /STACK only
  # reserves address space, and it is also the default size of threads created
  # with size 0 (the waveform writer) and the stack user DPI code expects, so
  # never reserve less than the 1 MiB Windows default.
  set(LLG_HOST_STACK_ESTIMATE_BYTES 655360 CACHE STRING
      "Estimated host stack for scheduler, one polled coroutine segment, depth guard, runtime helpers and DPI/libc headroom")
  set(LLG_HOST_STACK_RESERVE_BYTES 1048576)
  if(LLG_HOST_STACK_ESTIMATE_BYTES GREATER LLG_HOST_STACK_RESERVE_BYTES)
    set(LLG_HOST_STACK_RESERVE_BYTES ${LLG_HOST_STACK_ESTIMATE_BYTES})
  endif()
  target_link_options(sim PRIVATE /STACK:${LLG_HOST_STACK_RESERVE_BYTES})
else()
  target_link_libraries(sim PRIVATE m)
  if(UNIX AND NOT APPLE)
    target_link_libraries(sim PRIVATE dl)
  endif()
endif()
{WAVE_SETUP}
{DPI_LINK}
"#;

/// Per-source options for generated model translation units. GCC's
/// `-Wmisleading-indentation` costs time quadratic in file size (13.6 of 70.9 s
/// on a 10.8 MB model) and only reports source layout, which carries no meaning
/// in emitter output, so generated sources skip it; runtime sources keep it.
const MODEL_SOURCE_OPTIONS: &str = r#"if(CMAKE_C_COMPILER_ID MATCHES "GNU|Clang")
  set_source_files_properties({MODEL_SOURCES} PROPERTIES COMPILE_OPTIONS -Wno-misleading-indentation)
endif()
"#;

const RUNTIME_CMAKELISTS_TEMPLATE: &str = r#"cmake_minimum_required(VERSION 3.16)
project(llg_sim_runtime C)
set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
{OPTIMIZATION_SETUP}
if(NOT CMAKE_BUILD_TYPE)
  set(CMAKE_BUILD_TYPE Release)
endif()
set_source_files_properties(llg_co.c PROPERTIES COMPILE_DEFINITIONS LLG_CO_HOST_ALLOC=1)
add_library(llg_runtime STATIC {RUNTIME_SOURCES})
target_include_directories(llg_runtime PRIVATE ${CMAKE_SOURCE_DIR})
{WAVE_DEFINITION}
"#;

// Keep the configuration free of optimization flags so user flags come last.
// A normal variable shadows the cached user flags without accumulating prefixes
// when the same build directory is configured again.
const OPTIMIZATION_CMAKE_TEMPLATE: &str = r#"if(MSVC)
  set(CMAKE_C_FLAGS_RELEASE "/DNDEBUG")
  set(CMAKE_C_FLAGS "{MSVC_OPT_FLAG} /W3 ${CMAKE_C_FLAGS}")
else()
  set(CMAKE_C_FLAGS_RELEASE "-DNDEBUG")
  set(CMAKE_C_FLAGS "{GNU_OPT_FLAG} -Wall -Wno-unused-function ${CMAKE_C_FLAGS}")
endif()
"#;

/// Optimization levels for generated model and runtime C sources.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelOptLevel {
    /// Disable optimization (`/Od` on MSVC).
    O0,
    /// Basic optimization (`/O1` on MSVC).
    O1,
    /// Optimize for speed (`/O2` on MSVC).
    O2,
    /// Aggressive speed optimization (`/O2` on MSVC).
    O3,
    /// Optimize for size (`/O1` on MSVC).
    Os,
}

/// Default chosen by simulation time across the standard performance corpus.
pub const DEFAULT_MODEL_OPT_LEVEL: ModelOptLevel = ModelOptLevel::O3;

impl Default for ModelOptLevel {
    fn default() -> Self {
        DEFAULT_MODEL_OPT_LEVEL
    }
}

impl ModelOptLevel {
    /// Parse the level name used by `llg --model-opt-level`.
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "O0" => Ok(Self::O0),
            "O1" => Ok(Self::O1),
            "O2" => Ok(Self::O2),
            "O3" => Ok(Self::O3),
            "Os" => Ok(Self::Os),
            _ => Err("expected O0, O1, O2, O3 or Os"),
        }
    }

    /// GCC/Clang optimization flag. CMake selects the compiler family.
    pub const fn gnu_flag(self) -> &'static str {
        match self {
            Self::O0 => "-O0",
            Self::O1 => "-O1",
            Self::O2 => "-O2",
            Self::O3 => "-O3",
            Self::Os => "-Os",
        }
    }

    /// MSVC equivalent; O3 maps to O2 and Os maps to O1.
    pub const fn msvc_flag(self) -> &'static str {
        match self {
            Self::O0 => "/Od",
            Self::O1 | Self::Os => "/O1",
            Self::O2 | Self::O3 => "/O2",
        }
    }
}

fn optimization_setup(level: ModelOptLevel) -> String {
    OPTIMIZATION_CMAKE_TEMPLATE
        .replace("{GNU_OPT_FLAG}", level.gnu_flag())
        .replace("{MSVC_OPT_FLAG}", level.msvc_flag())
}

/// Compile `target` against the bundled zlib under `zlib/`. Its symbols are
/// prefixed (`Z_PREFIX`), and `<unistd.h>` supplies the POSIX I/O used by the
/// `gz*` API where zlib's own configure would have enabled it.
macro_rules! zlib_cmake {
    ($target:literal) => {
        concat!(
            "target_include_directories(",
            $target,
            " PRIVATE ${CMAKE_SOURCE_DIR}/zlib)\n",
            "target_compile_definitions(",
            $target,
            " PRIVATE Z_PREFIX)\n",
            "if(NOT WIN32)\n",
            "  target_compile_definitions(",
            $target,
            " PRIVATE Z_HAVE_UNISTD_H)\n",
            "endif()"
        )
    };
}

// Model sources such as the waveform self-test include `fstapi.h`, which
// includes `<zlib.h>`; `sim` must see the bundled, prefixed header rather
// than an unprefixed system copy (or none) in every runtime mode.
const WAVE_CMAKE: &str = concat!(
    r#"find_package(Threads REQUIRED)
target_link_libraries(sim PRIVATE Threads::Threads)
target_compile_definitions(sim PRIVATE LLG_WAVEFORM=1 FST_CONFIG_INCLUDE=\"fst_config.h\")
"#,
    zlib_cmake!("sim"),
    r#"
if(NOT LLG_RUNTIME_LIBRARY)
target_compile_definitions(llg_runtime PRIVATE LLG_WAVEFORM=1 FST_CONFIG_INCLUDE=\"fst_config.h\")
"#,
    zlib_cmake!("llg_runtime"),
    "\nendif()"
);
const RUNTIME_WAVE_DEFINITION: &str = concat!(
    "target_compile_definitions(llg_runtime PRIVATE LLG_WAVEFORM=1 FST_CONFIG_INCLUDE=\\\"fst_config.h\\\")\n",
    zlib_cmake!("llg_runtime")
);

const RUNTIME_CACHE_LOCK_TIMEOUT: Duration = Duration::from_secs(300);
const RUNTIME_CACHE_LOCK_POLL: Duration = Duration::from_millis(25);

/// Options for [`build_model_cmake_with_opts`].
#[derive(Default, Clone)]
pub struct CmakeBuildOpts {
    /// Explicit cmake `-G` generator backend (e.g. `"Ninja"`,
    /// `"Unix Makefiles"`).  Takes precedence over `$CMAKE_GENERATOR`; when
    /// `None` (or empty), a non-empty `$CMAKE_GENERATOR` is used, then
    /// [`DEFAULT_GENERATOR`].
    pub generator: Option<String>,
    /// Must match the configuration used by the emitter.
    pub value_config: super::value_backend::ValueConfig,
    /// Explicit GMP installation prefix; otherwise GMP_ROOT. Ignored for portable/legacy.
    pub gmp_root: Option<PathBuf>,
    /// Explicit user DPI-C libraries. Paths are passed to CMake as link
    /// items, so missing or non-files are rejected before configuration and
    /// no ambient linker search path can silently select a different ABI.
    pub dpi_libraries: Vec<PathBuf>,
    /// Optional C compiler launcher handed to CMake as
    /// `CMAKE_C_COMPILER_LAUNCHER` (for example `ccache` or `sccache`). `None`
    /// uses `$LLG_C_LAUNCHER`, and no launcher is selected when that is unset
    /// or empty; an explicit empty value suppresses the environment variable.
    pub launcher: Option<String>,
    /// Runtime archive cache root. `None` uses `$LLG_RUNTIME_CACHE_DIR`, then
    /// `build/llg-runtime-cache` under the current directory. Relative paths
    /// resolve from the current directory.
    pub runtime_cache_dir: Option<PathBuf>,
    /// C compiler. `None` uses `$LLG_CC`, then `$CC` (empty values are
    /// unset), then [`DEFAULT_C_COMPILER`].
    pub cc: Option<String>,
    /// Extra whitespace-separated C flags. `None` uses `$LLG_CFLAGS`; an
    /// explicit value replaces it rather than appending.
    pub cflags: Option<String>,
    /// Optimization for both model and runtime sources. User `cflags` (else
    /// `$LLG_CFLAGS`) follow this level and can override it. Source-only
    /// projects retain the selection. Defaults to [`DEFAULT_MODEL_OPT_LEVEL`].
    pub model_opt_level: ModelOptLevel,
    /// CMake program. `None` uses `$LLG_CMAKE`, then `cmake`.
    pub cmake: Option<String>,
    /// Parallel job count passed as `cmake --build --parallel <N>` to the
    /// runtime archive and model builds. `None` uses
    /// `$CMAKE_BUILD_PARALLEL_LEVEL` when it is a positive integer, then the
    /// host's available parallelism. `Some(0)` is treated as unset.
    pub build_jobs: Option<usize>,
}

/// Environment fallback for [`CmakeBuildOpts::launcher`]. `LLG_CC` must stay a
/// single program, so a compiler launcher such as `ccache` has its own variable.
pub const C_LAUNCHER_ENV: &str = "LLG_C_LAUNCHER";

/// Environment variable CMake itself reads for its default build parallelism;
/// honored here so users, CI and the test harness keep control.
pub const BUILD_PARALLEL_LEVEL_ENV: &str = "CMAKE_BUILD_PARALLEL_LEVEL";

/// Job count for `cmake --build --parallel`: a positive `explicit` value, else
/// a positive-integer `env_value` (the raw `$CMAKE_BUILD_PARALLEL_LEVEL`), else
/// `available`, else 1. Pure so the policy is testable without touching the
/// process environment.
pub fn resolve_build_jobs(
    explicit: Option<usize>,
    env_value: Option<&str>,
    available: Option<usize>,
) -> usize {
    explicit
        .filter(|jobs| *jobs > 0)
        .or_else(|| {
            env_value
                .and_then(|value| value.trim().parse::<usize>().ok())
                .filter(|jobs| *jobs > 0)
        })
        .or(available)
        .filter(|jobs| *jobs > 0)
        .unwrap_or(1)
}

fn build_jobs_for(opts: &CmakeBuildOpts) -> usize {
    resolve_build_jobs(
        opts.build_jobs,
        std::env::var(BUILD_PARALLEL_LEVEL_ENV).ok().as_deref(),
        std::thread::available_parallelism().ok().map(usize::from),
    )
}

/// `cmake --build <dir> --config Release --parallel <jobs> [--target <t>]`.
/// `--parallel` is generic across generators since CMake 3.12 (Makefiles and
/// Ninja jobs, MSBuild `/m`); the generated projects require 3.16.
fn build_command(cmake_prog: &str, build_dir: &Path, jobs: usize, target: Option<&str>) -> Command {
    let mut command = Command::new(cmake_prog);
    command
        .arg("--build")
        .arg(build_dir)
        .arg("--config")
        .arg(BUILD_TYPE)
        .arg("--parallel")
        .arg(jobs.to_string());
    if let Some(target) = target {
        command.arg("--target").arg(target);
    }
    command
}

/// Environment variable naming the runtime archive cache root.
pub const RUNTIME_CACHE_DIR_ENV: &str = "LLG_RUNTIME_CACHE_DIR";

/// Non-empty `$LLG_RUNTIME_CACHE_DIR`, for drivers that choose their own
/// default when it is unset.
pub fn runtime_cache_dir_from_env() -> Option<PathBuf> {
    std::env::var_os(RUNTIME_CACHE_DIR_ENV)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
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
    /// Generated source is missing or has an incompatible value ownership ABI.
    InvalidModelAbi(String),
    /// Invalid value/kernel selection or unavailable GMP input.
    InvalidValueConfig(String),
    /// A user-supplied DPI-C library is missing or cannot be represented
    /// safely in the generated CMake file.
    InvalidDpiLibrary { path: PathBuf, reason: String },
    /// The configured CMake program could not be launched.
    CmakeLaunch { program: String, source: io::Error },
    /// CMake configuration failed after one clean retry.
    Configure { command: String, output: String },
    /// CMake found no build program for the selected generator (for the
    /// default [`DEFAULT_GENERATOR`], Ninja is not installed).
    BuildProgramNotFound { generator: String, output: String },
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
            Self::InvalidValueConfig(value) => write!(f, "invalid value build configuration: {value}"),
            Self::InvalidModelAbi(value) => write!(f, "incompatible generated model value ABI `{value}`; regenerate the model for the selected value backend"),
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
            Self::BuildProgramNotFound { generator, output } => write!(
                f,
                "cmake found no build program for generator `{generator}`; {}, or select another generator with --generator or CMAKE_GENERATOR:\n{output}",
                if generator.starts_with("Ninja") {
                    "install Ninja (https://ninja-build.org; for example the `ninja-build` package, `brew install ninja`, or the Visual Studio C++ CMake tools) so `ninja` is on PATH"
                } else {
                    "install that generator's build tool"
                }
            ),
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
/// return the path of the resulting executable. Sources are written exactly
/// like [`generate_model_sources`] does (runtime + `extra`).
pub fn build_model_cmake(out_dir: &Path, extra: &[(&str, &str)]) -> Result<PathBuf, BuildError> {
    build_model_cmake_with_opts(out_dir, extra, &CmakeBuildOpts::default())
}

/// Build the simulation model in `out_dir` with CMake and per-call options;
/// returns the path of the resulting executable (`<out_dir>/build/bin/sim`,
/// with the host executable suffix).
pub fn build_model_cmake_with_opts(
    out_dir: &Path,
    extra: &[(&str, &str)],
    opts: &CmakeBuildOpts,
) -> Result<PathBuf, BuildError> {
    validate_dpi_libraries(opts)?;
    generate_model_sources_with_opts(out_dir, extra, opts)?;

    let cc = resolve_cc(opts);
    let flags = c_flags(opts)?;
    let build_dir = out_dir.join("build");
    let cmake_prog = resolve_cmake(opts);
    let waveform = waveform_enabled(extra);
    let jobs = build_jobs_for(opts);
    let generator = generator_for(opts);
    let launcher = resolve_launcher(opts);
    let cache_root = runtime_cache_root(opts)?;
    let compiler = compiler_probe::facts(&cc, &cache_root);
    let toolchain = toolchain_seed::Toolchain {
        cmake: &cmake_prog,
        generator: &generator,
        cc: &cc,
        flags: &flags,
        launcher: &launcher,
        compiler_identity: &compiler.identity,
        compiler_target: &compiler.target,
    };
    let runtime_library = prepare_runtime_cache(waveform, &toolchain, jobs, opts)?;

    // Drop an incompatible CMake build tree so configure starts clean: no
    // cache means a previous configure died midway; a generator mismatch
    // would make cmake refuse the directory outright.  A compatible tree is
    // kept — reconfigure + build are incremental.
    remove_incompatible_build_dir(&build_dir, &generator);

    let runtime_library_arg = format!("-DLLG_RUNTIME_LIBRARY={}", runtime_library.display());
    run_configure(
        out_dir,
        &build_dir,
        &toolchain,
        &[runtime_library_arg],
        &cache_root,
    )?;
    let launch_error = |source| BuildError::CmakeLaunch {
        program: cmake_prog.clone(),
        source,
    };

    // Build.
    let mut build_cmd = build_command(&cmake_prog, &build_dir, jobs, None);
    let output = build_cmd.output().map_err(launch_error)?;
    if !output.status.success() {
        return Err(BuildError::Compile {
            output: output_tail(&output),
        });
    }

    find_sim_exe(&build_dir.join("bin"))
}

/// Write the runtime sources plus `extra` (the generated `model.c`)
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
    validate_model_abi_for(extra, opts.value_config)?;
    let guard = value::guard_header(opts)?;
    validate_dpi_libraries(opts)?;
    super::write_sim_sources(out_dir, extra, opts.value_config)?;
    std::fs::write(out_dir.join("llg_value_build.h"), guard).map_err(|source| BuildError::Io {
        action: "write",
        path: out_dir.join("llg_value_build.h"),
        source,
    })?;
    let waveform = waveform_enabled(extra);
    if waveform {
        super::rt::write_waveform_sources(out_dir)?;
    }
    // Written even when a cached runtime will be linked, so the project also
    // builds standalone.
    let bundled_gmp = value::uses_bundled_gmp(opts)?;
    if bundled_gmp {
        super::rt::write_bundled_gmp_sources(out_dir)?;
    }
    write_cmakelists(out_dir, extra, waveform, opts)?;
    prune_stale_entries_for(
        out_dir,
        extra,
        waveform,
        bundled_gmp,
        opts.value_config.backend,
    );
    Ok(())
}

/// File names [`super::write_sim_sources`] always writes (must mirror its
/// fixed list there) plus this module's own `CMakeLists.txt`.
const FIXED_SOURCE_NAMES: [&str; 23] = [
    "llg_rt.h",
    "llg_rt.c",
    "llg_value.h",
    "llg_value.c",
    "llg_random.h",
    "llg_random.c",
    "llg_rng.h",
    "llg_rng.c",
    "llg_co.h",
    "llg_co.c",
    "vpi_user.h",
    "llg_vpi.h",
    "llg_vpi.c",
    "llg_container.h",
    "llg_container.c",
    "llg_string.h",
    "llg_string.c",
    "svdpi.h",
    "llg_compiler.h",
    "llg_platform.h",
    "llg_platform_native.h",
    "CMakeLists.txt",
    "llg_value_build.h",
];

/// Delete every direct child of `out_dir` that is neither a current source
/// file nor the `build/` directory (rm-all-then-write semantics for the flat
/// source set).  Refuses to run unless the canonicalized `out_dir` is an
/// absolute path at least two levels below the filesystem root: unresolvable
/// (e.g. empty) paths, `/`, and shallow roots like `/tmp` are rejected, while
/// real callers (`<out-dir>/sim/<design>`, tempdir subdirectories) are
/// unaffected.
fn prune_stale_entries_for(
    out_dir: &Path,
    extra: &[(&str, &str)],
    waveform: bool,
    bundled_gmp: bool,
    backend: super::value_backend::ValueBackend,
) {
    let canonical = match crate::ffi::platform::canonicalize(out_dir) {
        Ok(p) => p,
        Err(_) => return,
    };
    // canonicalize() already yields absolute paths; the explicit check keeps
    // the guard's intent obvious.  Depth >= 3 = <root>/<level>/<name>.
    if !canonical.is_absolute() || canonical.components().count() < 3 {
        return;
    }
    let mut expected: Vec<&str> = FIXED_SOURCE_NAMES.to_vec();
    expected.extend(
        super::rt::value_backend_sources(backend)
            .iter()
            .filter_map(|(name, _)| name.split('/').next()),
    );
    let waveform_sources: &[(&str, &str)] = if waveform {
        super::rt::waveform_sources()
    } else {
        &[]
    };
    let gmp_sources: &[(&str, &str)] = if bundled_gmp {
        super::rt::gmp::bundled_gmp_sources()
    } else {
        &[]
    };
    expected.extend(
        waveform_sources
            .iter()
            .chain(gmp_sources)
            .filter_map(|(name, _)| name.split('/').next()),
    );
    expected.extend(extra.iter().map(|(name, _)| *name));
    // Compare paths by component: on Windows the relative path of a
    // written `value/backend.h` is spelled `value\backend.h`.
    let nested = super::rt::value_backend_sources(backend)
        .iter()
        .chain(waveform_sources)
        .chain(gmp_sources)
        .filter(|(name, _)| name.contains('/'))
        .map(|(name, _)| Path::new(*name))
        .collect::<Vec<_>>();
    for directory in ["value", "value_gmp", "zlib", "gmp"] {
        let mut paths = Vec::new();
        collect_paths(&out_dir.join(directory), &mut paths);
        for path in paths {
            if path.is_file()
                && path
                    .strip_prefix(out_dir)
                    .is_ok_and(|relative| !nested.contains(&relative))
            {
                let _ = std::fs::remove_file(path);
            }
        }
    }
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

/// Remove `build_dir` unless it holds a readable cache for `generator`: a
/// missing cache means an earlier configure died midway, and CMake refuses a
/// tree configured with another generator. A tree from an older default
/// generator is therefore rebuilt once and then matches.
fn remove_incompatible_build_dir(build_dir: &Path, generator: &str) {
    if build_dir.exists() && cached_generator(build_dir).as_deref() != Some(generator) {
        remove_dir_all_quiet(build_dir);
    }
}

/// The first-configure command. The build type is always explicit: on MSVC
/// CMake initializes `CMAKE_BUILD_TYPE` to Debug for single-config generators
/// (Windows-MSVC.cmake), which the generated projects' `if(NOT CMAKE_BUILD_TYPE)`
/// default then respects, giving `/Od` and Debug linker flags under Ninja.
fn configure_command(
    source: &Path,
    build_dir: &Path,
    toolchain: &toolchain_seed::Toolchain<'_>,
    extra: &[String],
    seed_args: &[String],
) -> Command {
    let mut command = Command::new(toolchain.cmake);
    command
        .arg("-S")
        .arg(source)
        .arg("-B")
        .arg(build_dir)
        .args(toolchain_seed::toolchain_args(
            toolchain.generator,
            toolchain.launcher,
            toolchain.cc,
            toolchain.flags,
        ))
        .arg(format!("-DCMAKE_BUILD_TYPE={BUILD_TYPE}"))
        .args(extra)
        .args(seed_args);
    command
}

/// Configure `source` into `build_dir` with `toolchain` plus `extra`
/// definitions. A fresh tree is first seeded with the toolchain detection
/// results under `seed_root` (see [`toolchain_seed`]). A failed configure
/// leaves the tree unusable no matter the cause (poisoned/stale cache,
/// half-written state, changed toolchain, a bad seed), so it is retried
/// exactly once from scratch without a seed, except when CMake found no build
/// program for the generator, which a retry cannot fix. A seed whose configure
/// failed where the clean retry succeeded is rejected for later builds.
fn run_configure(
    source: &Path,
    build_dir: &Path,
    toolchain: &toolchain_seed::Toolchain<'_>,
    extra: &[String],
    seed_root: &Path,
) -> Result<(), BuildError> {
    let command =
        |seed_args: &[String]| configure_command(source, build_dir, toolchain, extra, seed_args);
    let launch_error = |source| BuildError::CmakeLaunch {
        program: toolchain.cmake.to_owned(),
        source,
    };
    let seed = if build_dir.exists() {
        None
    } else {
        toolchain_seed::prepare(seed_root, toolchain)
    };
    let seed_args = seed.as_ref().and_then(|seed| seed.apply(build_dir));
    let mut configure = command(seed_args.as_deref().unwrap_or_default());
    let mut output = configure.output().map_err(launch_error)?;
    if !output.status.success() && !missing_build_program(&output) {
        remove_dir_all_quiet(build_dir);
        configure = command(&[]);
        output = configure.output().map_err(launch_error)?;
        if output.status.success() && seed_args.is_some() {
            if let Some(seed) = &seed {
                seed.reject("a seeded configure failed where a clean configure succeeded");
            }
        }
    }
    if output.status.success() {
        Ok(())
    } else {
        Err(configure_error(&configure, toolchain.generator, &output))
    }
}

fn remove_dir_all_quiet(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Emit `CMakeLists.txt` for the source set `extra` plus the runtime.
fn write_cmakelists(
    out_dir: &Path,
    extra: &[(&str, &str)],
    waveform: bool,
    opts: &CmakeBuildOpts,
) -> Result<(), BuildError> {
    let model_sources: Vec<&str> = extra
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| name.ends_with(".c"))
        .collect();
    let model_source_options = if model_sources.is_empty() {
        String::new()
    } else {
        MODEL_SOURCE_OPTIONS.replace("{MODEL_SOURCES}", &model_sources.join(" "))
    };
    let cmakelists = CMAKELISTS_TEMPLATE
        .replace(
            "{OPTIMIZATION_SETUP}",
            &optimization_setup(opts.model_opt_level),
        )
        .replace("{MODEL_SOURCE_OPTIONS}", &model_source_options)
        .replace("{MODEL_SOURCES}", &model_sources.join(" "))
        .replace(
            "{RUNTIME_SOURCES}",
            &runtime_source_names_for(waveform, opts.value_config.backend).join(" "),
        )
        .replace("{WAVE_SETUP}", if waveform { WAVE_CMAKE } else { "" })
        .replace("{DPI_LINK}", &dpi_link_setup(opts)?)
        + &value::cmake_setup(opts, "sim")?;
    let cmakelists_path = out_dir.join("CMakeLists.txt");
    std::fs::write(&cmakelists_path, cmakelists).map_err(|source| BuildError::Io {
        action: "write",
        path: cmakelists_path,
        source,
    })
}

fn validate_dpi_libraries(opts: &CmakeBuildOpts) -> Result<(), BuildError> {
    for path in &opts.dpi_libraries {
        let canonical = canonical_dpi_library(path)?;
        // PLI 1.0 TF/ACC is unsupported by design: name it here rather than
        // fail later with an unresolved `tf_*`/`acc_*` symbol or a table
        // that is never read.
        match legacy_pli::find_legacy_pli_symbol(&canonical) {
            Ok(None) => {}
            Ok(Some(symbol)) => {
                return Err(BuildError::InvalidDpiLibrary {
                    path: path.to_path_buf(),
                    reason: format!(
                        "unsupported: the PLI 1.0 TF/ACC interface (`{symbol}`) is not supported \
                         by llg; llg ships no veriuser.h or acc_user.h, no veriusertfs \
                         registration and no tf_*/acc_* routines, so port the library to VPI \
                         (vlog_startup_routines) or DPI-C"
                    ),
                });
            }
            Err(error) => {
                return Err(BuildError::InvalidDpiLibrary {
                    path: path.to_path_buf(),
                    reason: format!("cannot read library: {error}"),
                });
            }
        }
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
    // Ordinary spelling: a Windows verbatim `\\?\` prefix would reach the
    // CMake link line as `//?/C:/...`.
    let canonical = crate::ffi::platform::canonicalize(path).map_err(|error| {
        BuildError::InvalidDpiLibrary {
            path: path.to_path_buf(),
            reason: format!("cannot resolve path: {error}"),
        }
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
/// Packed widths and host-stack headroom do not affect the dynamic value
/// layout or the cache key.
fn prepare_runtime_cache(
    waveform: bool,
    toolchain: &toolchain_seed::Toolchain<'_>,
    jobs: usize,
    opts: &CmakeBuildOpts,
) -> Result<PathBuf, BuildError> {
    let cmake_prog = toolchain.cmake;
    let key = format!(
        "{}-{}",
        runtime_cache_key_with_compiler(
            waveform,
            toolchain.cc,
            toolchain.flags,
            cmake_prog,
            opts,
            toolchain.compiler_identity,
            toolchain.compiler_target,
        ),
        value::identity(opts)?
    );
    let cache_root = runtime_cache_root(opts)?;
    let entry = cache_root.join(&key);
    if let Some(library) = cached_runtime_library(&entry, &key) {
        return Ok(library);
    }

    std::fs::create_dir_all(&cache_root).map_err(|source| BuildError::Io {
        action: "create runtime cache",
        path: cache_root.clone(),
        source,
    })?;
    let _lock = RuntimeCacheLock::acquire(&entry)?;
    if let Some(library) = cached_runtime_library(&entry, &key) {
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
    super::write_sim_sources(&entry, &[], opts.value_config)?;
    std::fs::write(entry.join("llg_value_build.h"), value::guard_header(opts)?).map_err(
        |source| BuildError::Io {
            action: "write",
            path: entry.join("llg_value_build.h"),
            source,
        },
    )?;
    if waveform {
        super::rt::write_waveform_sources(&entry)?;
    }
    if value::uses_bundled_gmp(opts)? {
        super::rt::write_bundled_gmp_sources(&entry)?;
    }
    let runtime_sources = runtime_source_names_for(waveform, opts.value_config.backend).join(" ");
    let cmakelists = RUNTIME_CMAKELISTS_TEMPLATE
        .replace(
            "{OPTIMIZATION_SETUP}",
            &optimization_setup(opts.model_opt_level),
        )
        .replace("{RUNTIME_SOURCES}", &runtime_sources)
        .replace(
            "{WAVE_DEFINITION}",
            if waveform {
                RUNTIME_WAVE_DEFINITION
            } else {
                ""
            },
        )
        + &value::cmake_setup(opts, "llg_runtime")?;
    let cmakelists_path = entry.join("CMakeLists.txt");
    std::fs::write(&cmakelists_path, cmakelists).map_err(|source| BuildError::Io {
        action: "write runtime cache project",
        path: cmakelists_path,
        source,
    })?;

    let build_dir = entry.join("build");
    remove_incompatible_build_dir(&build_dir, toolchain.generator);
    run_configure(&entry, &build_dir, toolchain, &[], &cache_root)?;
    let launch_error = |source| BuildError::CmakeLaunch {
        program: cmake_prog.to_owned(),
        source,
    };

    let mut build = build_command(cmake_prog, &build_dir, jobs, Some("llg_runtime"));
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
    std::fs::write(&ready_path, &key).map_err(|source| BuildError::Io {
        action: "mark runtime cache ready",
        path: ready_path,
        source,
    })?;
    Ok(library)
}

fn runtime_source_names_for(
    waveform: bool,
    backend: super::value_backend::ValueBackend,
) -> Vec<&'static str> {
    let mut sources = vec![
        "llg_value.c",
        "llg_rng.c",
        "llg_co.c",
        "llg_rt.c",
        "llg_random.c",
        "llg_vpi.c",
        "llg_container.c",
        "llg_string.c",
    ];
    sources.extend(
        super::rt::value_backend_sources(backend)
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| name.ends_with(".c")),
    );
    if waveform {
        sources.extend(
            super::rt::waveform_sources()
                .iter()
                .map(|(name, _)| *name)
                .filter(|name| name.ends_with(".c")),
        );
    }
    sources
}

/// Resolve the runtime cache: explicit option, then `$LLG_RUNTIME_CACHE_DIR`,
/// then `build/llg-runtime-cache`, all relative to the current directory. The
/// result is absolute because it is handed to CMake as `LLG_RUNTIME_LIBRARY`.
fn runtime_cache_root(opts: &CmakeBuildOpts) -> Result<PathBuf, BuildError> {
    let cwd = std::env::current_dir().map_err(|source| BuildError::Io {
        action: "resolve current directory for runtime cache",
        path: PathBuf::from("."),
        source,
    })?;
    Ok(runtime_cache_root_with_override(
        &cwd,
        opts.runtime_cache_dir
            .clone()
            .or_else(runtime_cache_dir_from_env),
    ))
}

fn runtime_cache_root_with_override(base: &Path, override_root: Option<PathBuf>) -> PathBuf {
    match override_root {
        Some(path) if path.is_absolute() => path,
        Some(path) if !path.as_os_str().is_empty() => base.join(path),
        _ => base.join("build/llg-runtime-cache"),
    }
}

#[cfg(test)]
fn runtime_cache_key(
    waveform: bool,
    cc: &str,
    flags: &str,
    cmake_prog: &str,
    opts: &CmakeBuildOpts,
) -> String {
    let compiler = compiler_probe::facts(cc, &std::env::temp_dir());
    runtime_cache_key_with_compiler(
        waveform,
        cc,
        flags,
        cmake_prog,
        opts,
        &compiler.identity,
        &compiler.target,
    )
}

fn runtime_cache_key_with_compiler(
    waveform: bool,
    cc: &str,
    flags: &str,
    cmake_prog: &str,
    opts: &CmakeBuildOpts,
    compiler: &str,
    target: &str,
) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    let generator = generator_for(opts);
    let launcher = resolve_launcher(opts);
    for text in [
        RUNTIME_CMAKELISTS_TEMPLATE,
        include_str!("build/value.rs"),
        OPTIMIZATION_CMAKE_TEMPLATE,
        opts.model_opt_level.gnu_flag(),
        opts.model_opt_level.msvc_flag(),
        RUNTIME_WAVE_DEFINITION,
        super::rt::runtime_sources().0,
        super::rt::runtime_sources().1,
        super::rt::value_sources_for(opts.value_config.backend).0,
        super::rt::value_sources_for(opts.value_config.backend).1,
        super::rt::random_sources().0,
        super::rt::random_sources().1,
        super::rt::rng_sources().0,
        super::rt::rng_sources().1,
        super::rt::coroutine_sources().0,
        super::rt::coroutine_sources().1,
        super::rt::vpi_sources().0,
        super::rt::vpi_sources().1,
        super::rt::vpi_bridge_header(),
        super::rt::container_sources().0,
        super::rt::container_sources().1,
        super::rt::string_sources().0,
        super::rt::string_sources().1,
        include_str!("../../vendor/slang/external/ieee1800/svdpi.h"),
        cc,
        flags,
        cmake_prog,
        BUILD_TYPE,
        compiler,
        target,
        &generator,
        &launcher,
        std::env::consts::OS,
        std::env::consts::ARCH,
    ] {
        for byte in text.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    for (name, source) in super::rt::platform_headers()
        .iter()
        .chain(super::rt::value_backend_sources(opts.value_config.backend))
    {
        for byte in name.bytes().chain(source.bytes()) {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    hash = (hash ^ u64::from(opts.value_config.kernel.selector())).wrapping_mul(0x100000001b3);
    if waveform {
        for (_, source) in super::rt::waveform_sources() {
            for byte in source.as_bytes() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
        }
    }
    format!(
        "owned-v{}-wave{}-{hash:016x}",
        opts.value_config.backend.abi(),
        u8::from(waveform)
    )
}

fn find_runtime_library(build_dir: &Path) -> Option<PathBuf> {
    let mut found = Vec::new();
    collect_named_files(build_dir, "libllg_runtime.a", &mut found);
    collect_named_files(build_dir, "llg_runtime.lib", &mut found);
    found.sort();
    found.into_iter().next()
}

fn cached_runtime_library(entry: &Path, key: &str) -> Option<PathBuf> {
    (std::fs::read_to_string(entry.join("ready")).ok().as_deref() == Some(key))
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
        let (path, file) = Self::open(entry)?;
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

    /// The lock when it is free now; `None` while another process holds it.
    fn try_acquire(entry: &Path) -> Result<Option<Self>, BuildError> {
        let (path, file) = Self::open(entry)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(source)) => Err(BuildError::Io {
                action: "lock runtime cache",
                path,
                source,
            }),
        }
    }

    fn open(entry: &Path) -> Result<(PathBuf, File), BuildError> {
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
        Ok((path, file))
    }
}

/// Hand-written C probes may omit model metadata. Generated model translation
/// units must opt into the ownership ABI explicitly; old source is never guessed
/// compatible from a default packed capacity.
#[cfg(test)]
fn validate_model_abi(extra: &[(&str, &str)]) -> Result<(), BuildError> {
    validate_model_abi_for(extra, super::value_backend::ValueConfig::default())
}

fn validate_model_abi_for(
    extra: &[(&str, &str)],
    config: super::value_backend::ValueConfig,
) -> Result<(), BuildError> {
    for (name, source) in extra {
        let mut abi = None;
        for line in source.lines() {
            if let Some(value) = line.strip_prefix("#define LLG_MODEL_VALUE_ABI ") {
                let value = value.trim();
                let parsed = value.parse::<u32>().ok();
                if parsed != Some(config.backend.abi()) || abi.is_some() {
                    return Err(BuildError::InvalidModelAbi(value.to_owned()));
                }
                abi = parsed;
            }
        }
        for (guard, expected) in [
            (
                "#define LLG_MODEL_VALUE_BACKEND ",
                config.backend.selector(),
            ),
            (
                "#define LLG_MODEL_COMPACT_KERNELS ",
                config.kernel.selector(),
            ),
        ] {
            let values = source
                .lines()
                .filter_map(|line| line.strip_prefix(guard))
                .collect::<Vec<_>>();
            if values.len() > 1
                || values
                    .first()
                    .is_some_and(|value| value.trim().parse::<u8>().ok() != Some(expected))
            {
                return Err(BuildError::InvalidValueConfig(format!(
                    "generated {name} selection differs from build; regenerate model"
                )));
            }
            if config.backend == super::value_backend::ValueBackend::Compact
                && abi.is_some()
                && values.is_empty()
            {
                return Err(BuildError::InvalidValueConfig(format!(
                    "missing {guard} in {name}; regenerate model"
                )));
            }
        }
        let generated = Path::new(name)
            .file_name()
            .is_some_and(|base| base == "model.c")
            || source.starts_with("// llg-generated C11 model");
        if generated && abi.is_none() {
            return Err(BuildError::InvalidModelAbi(format!("missing in {name}")));
        }
    }
    Ok(())
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
/// Captured model output with the host's native newline rewritten to LF
/// ([`crate::ffi::platform::native_text_to_lf`]), for unit tests that run
/// generated models with their own process code and compare LF oracles.
/// The emitter's tests reach the platform layer through this helper because
/// `sim::emit_c` must not reference `crate::ffi`.
#[cfg(test)]
pub(crate) fn model_output_to_lf(output: std::process::Output) -> std::process::Output {
    std::process::Output {
        status: output.status,
        stdout: crate::ffi::platform::native_text_to_lf(output.stdout),
        stderr: crate::ffi::platform::native_text_to_lf(output.stderr),
    }
}

pub fn cmake_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new(default_cmake())
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Generator used when neither [`CmakeBuildOpts::generator`] nor
/// `$CMAKE_GENERATOR` names one, on every host. Ninja builds the many small
/// model and runtime translation units with less per-invocation overhead
/// than Makefiles or MSBuild, and one default keeps cache keys and build
/// trees uniform across platforms. Model builds therefore require Ninja
/// unless another generator is selected explicitly.
pub const DEFAULT_GENERATOR: &str = "Ninja";

/// `-G` value: explicit option > `$CMAKE_GENERATOR` > [`DEFAULT_GENERATOR`].
fn generator_for(opts: &CmakeBuildOpts) -> String {
    generator_from(
        opts.generator.as_deref(),
        std::env::var("CMAKE_GENERATOR").ok().as_deref(),
    )
}

/// Pure generator precedence; empty values count as unset.
fn generator_from(explicit: Option<&str>, env_value: Option<&str>) -> String {
    explicit
        .filter(|name| !name.is_empty())
        .or(env_value.filter(|name| !name.is_empty()))
        .unwrap_or(DEFAULT_GENERATOR)
        .to_owned()
}

/// Whether configure output reports that CMake found no build program for
/// the selected generator (for Ninja: `ninja` is not installed). Retrying
/// from scratch cannot fix that, so the caller reports it directly.
fn missing_build_program(output: &std::process::Output) -> bool {
    [&output.stderr, &output.stdout].iter().any(|bytes| {
        let text = String::from_utf8_lossy(bytes);
        text.contains("CMAKE_MAKE_PROGRAM is not set")
            || text.contains("unable to find a build program corresponding to")
    })
}

/// Error for a configure that failed with `output`.
fn configure_error(
    command: &Command,
    generator: &str,
    output: &std::process::Output,
) -> BuildError {
    if missing_build_program(output) {
        return BuildError::BuildProgramNotFound {
            generator: generator.to_owned(),
            output: output_tail(output),
        };
    }
    BuildError::Configure {
        command: format!("{command:?}"),
        output: output_tail(output),
    }
}

/// Launcher precedence shared by the environment fallback and the tests:
/// explicit option (an empty one means none) > `$LLG_C_LAUNCHER` > none.
fn launcher_from(explicit: Option<&str>, env_value: Option<&str>) -> String {
    explicit
        .or(env_value)
        .map(str::trim)
        .unwrap_or_default()
        .to_owned()
}

/// `CMAKE_C_COMPILER_LAUNCHER` value; empty selects no launcher.
fn resolve_launcher(opts: &CmakeBuildOpts) -> String {
    launcher_from(
        opts.launcher.as_deref(),
        std::env::var(C_LAUNCHER_ENV).ok().as_deref(),
    )
}

/// C compiler for generated models when neither an option nor `$LLG_CC`/`$CC`
/// selects one. Models are built and run on the host that runs `llg`, so the
/// host platform's native compiler is the default: MSVC `cl` on Windows (no
/// `cc` exists there unless a GCC distribution happens to be on `PATH`, and
/// CMake itself defaults to MSVC), `cc` elsewhere.
pub const DEFAULT_C_COMPILER: &str = if cfg!(windows) { "cl" } else { "cc" };

/// Explicit option, else `$LLG_CC`, else `$CC`, else [`DEFAULT_C_COMPILER`].
fn resolve_cc(opts: &CmakeBuildOpts) -> String {
    select_cc(
        opts.cc.as_deref(),
        std::env::var("LLG_CC").ok().as_deref(),
        std::env::var("CC").ok().as_deref(),
    )
}

/// The compiler precedence of [`resolve_cc`] without the environment. Empty
/// variables count as unset, as in the `llg` driver.
fn select_cc(explicit: Option<&str>, llg_cc: Option<&str>, cc: Option<&str>) -> String {
    explicit
        .or(llg_cc.filter(|value| !value.is_empty()))
        .or(cc.filter(|value| !value.is_empty()))
        .unwrap_or(DEFAULT_C_COMPILER)
        .to_owned()
}

/// Explicit option, else `$LLG_CMAKE`, else `cmake`.
fn resolve_cmake(opts: &CmakeBuildOpts) -> String {
    opts.cmake.clone().unwrap_or_else(default_cmake)
}

fn default_cmake() -> String {
    std::env::var("LLG_CMAKE").unwrap_or_else(|_| "cmake".to_string())
}

/// `-DCMAKE_C_FLAGS` payload: explicit flags, else `$LLG_CFLAGS`. The generated
/// project prefixes compiler-specific optimization and warnings.
fn c_flags(opts: &CmakeBuildOpts) -> Result<String, BuildError> {
    resolve_c_flags(
        opts.cflags.as_deref(),
        std::env::var("LLG_CFLAGS").ok().as_deref(),
    )
}

fn resolve_c_flags(explicit: Option<&str>, env_value: Option<&str>) -> Result<String, BuildError> {
    let mut flags = Vec::new();
    if let Some(extra) = explicit.or(env_value) {
        for flag in extra.split_whitespace() {
            if flag.contains('"') {
                return Err(BuildError::InvalidCompilerFlag(flag.to_string()));
            }
            flags.push(flag);
        }
    }
    Ok(flags.join(" "))
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

/// File name of the generated executable. CMake builds the model for the host
/// running llg, so the host suffix applies (`sim.exe` on Windows).
fn sim_exe_name() -> String {
    format!("sim{}", std::env::consts::EXE_SUFFIX)
}

/// Locate the built executable: `<build>/bin/sim` first, then a recursive
/// search under `<build>/bin/` (multi-config generators may add per-config
/// subdirectories).  Deterministic order on all paths.
fn find_sim_exe(bin_dir: &Path) -> Result<PathBuf, BuildError> {
    let name = sim_exe_name();
    let direct = bin_dir.join(&name);
    if direct.is_file() {
        return Ok(direct);
    }
    let mut found = Vec::new();
    collect_named_files(bin_dir, &name, &mut found);
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
    fn selected_runtime_keys_and_stale_ready_markers_are_rejected() {
        use crate::sim::value_backend::{CompactKernel, ValueBackend, ValueConfig};
        let key = |config| {
            runtime_cache_key_with_compiler(
                false,
                "cc",
                "",
                "cmake",
                &CmakeBuildOpts {
                    value_config: config,
                    ..Default::default()
                },
                "cc",
                "target",
            )
        };
        let legacy = key(ValueConfig::default());
        let portable = key(ValueConfig {
            backend: ValueBackend::Compact,
            kernel: CompactKernel::Portable,
        });
        let gmp = key(ValueConfig {
            backend: ValueBackend::Compact,
            kernel: CompactKernel::Gmp,
        });
        assert_ne!(legacy, portable);
        assert_ne!(portable, gmp);
        let directory = std::env::temp_dir().join(format!("llg-v07-ready-{}", std::process::id()));
        std::fs::create_dir_all(directory.join("build")).unwrap();
        std::fs::write(directory.join("build/libllg_runtime.a"), "stale").unwrap();
        std::fs::write(directory.join("ready"), "ready\n").unwrap();
        assert!(cached_runtime_library(&directory, &legacy).is_none());
        std::fs::write(directory.join("ready"), &legacy).unwrap();
        assert!(cached_runtime_library(&directory, &portable).is_none());
        assert!(cached_runtime_library(&directory, &legacy).is_some());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn stale_pruning_keeps_written_nested_value_sources() {
        use crate::sim::value_backend::{ValueBackend, ValueConfig};
        for backend in [ValueBackend::Legacy, ValueBackend::Compact] {
            let directory = std::env::temp_dir().join(format!(
                "llg-prune-nested-{}-{}",
                std::process::id(),
                backend.abi()
            ));
            let _ = std::fs::remove_dir_all(&directory);
            std::fs::create_dir_all(directory.join("value")).unwrap();
            std::fs::write(directory.join("value/stale.h"), "stale").unwrap();
            let config = ValueConfig {
                backend,
                ..Default::default()
            };
            super::super::write_sim_sources(&directory, &[], config).unwrap();
            super::super::rt::write_waveform_sources(&directory).unwrap();
            std::fs::write(directory.join("zlib/stale.c"), "stale").unwrap();
            prune_stale_entries_for(&directory, &[], true, false, backend);
            let written = super::super::rt::value_backend_sources(backend)
                .iter()
                .chain(super::super::rt::waveform_sources());
            for (name, _) in written {
                assert!(directory.join(name).is_file(), "{name} was pruned");
            }
            assert!(!directory.join("value/stale.h").exists());
            assert!(!directory.join("zlib/stale.c").exists());
            // Without waveform tasks the bundled zlib directory is stale.
            prune_stale_entries_for(&directory, &[], false, false, backend);
            assert!(!directory.join("zlib").exists());
            // Bundled GMP sources follow the same rule.
            super::super::rt::write_bundled_gmp_sources(&directory).unwrap();
            std::fs::write(directory.join("gmp/mpn/generic/stale.c"), "stale").unwrap();
            prune_stale_entries_for(&directory, &[], false, true, backend);
            for (name, _) in super::super::rt::gmp::bundled_gmp_sources() {
                assert!(directory.join(name).is_file(), "{name} was pruned");
            }
            assert!(!directory.join("gmp/mpn/generic/stale.c").exists());
            prune_stale_entries_for(&directory, &[], false, false, backend);
            assert!(!directory.join("gmp").exists());
            std::fs::remove_dir_all(&directory).unwrap();
        }
    }

    #[test]
    fn model_optimization_levels_parse_and_map_to_compilers() {
        for (name, level, gnu, msvc) in [
            ("O0", ModelOptLevel::O0, "-O0", "/Od"),
            ("O1", ModelOptLevel::O1, "-O1", "/O1"),
            ("O2", ModelOptLevel::O2, "-O2", "/O2"),
            ("O3", ModelOptLevel::O3, "-O3", "/O2"),
            ("Os", ModelOptLevel::Os, "-Os", "/O1"),
        ] {
            assert_eq!(ModelOptLevel::parse(name), Ok(level));
            assert_eq!(level.gnu_flag(), gnu);
            assert_eq!(level.msvc_flag(), msvc);
        }
        for invalid in ["", "1", "-O2", "O4", "o3", "Oz"] {
            assert!(ModelOptLevel::parse(invalid).is_err());
        }
        assert_eq!(
            CmakeBuildOpts::default().model_opt_level,
            DEFAULT_MODEL_OPT_LEVEL
        );
    }

    #[test]
    fn explicit_cflags_replace_environment_and_follow_the_level() {
        assert_eq!(resolve_c_flags(None, None).unwrap(), "");
        assert_eq!(resolve_c_flags(None, Some("-O1  -g")).unwrap(), "-O1 -g");
        assert_eq!(resolve_c_flags(Some("-O0"), Some("-O3")).unwrap(), "-O0");
        assert_eq!(resolve_c_flags(Some(""), Some("-O3")).unwrap(), "");
        assert!(resolve_c_flags(None, Some("-DX=\"y\"")).is_err());
        let setup = optimization_setup(ModelOptLevel::O2);
        assert!(setup.contains("set(CMAKE_C_FLAGS_RELEASE \"-DNDEBUG\")"));
        assert!(setup.contains("set(CMAKE_C_FLAGS_RELEASE \"/DNDEBUG\")"));
        assert!(setup.contains("-O2 -Wall -Wno-unused-function ${CMAKE_C_FLAGS}"));
        assert!(setup.contains("/O2 /W3 ${CMAKE_C_FLAGS}"));
    }

    #[test]
    fn runtime_cache_separates_model_optimization_levels_and_user_flags() {
        let opts = CmakeBuildOpts::default();
        let key = |opts: &CmakeBuildOpts, flags| {
            runtime_cache_key_with_compiler(false, "cc", flags, "cmake", opts, "cc", "target")
        };
        let o1 = CmakeBuildOpts {
            model_opt_level: ModelOptLevel::O1,
            ..opts.clone()
        };
        let o2 = CmakeBuildOpts {
            model_opt_level: ModelOptLevel::O2,
            ..opts.clone()
        };
        let o3 = CmakeBuildOpts {
            model_opt_level: ModelOptLevel::O3,
            ..opts
        };
        assert_ne!(key(&o1, ""), key(&o2, ""));
        assert_ne!(key(&o2, ""), key(&o3, ""));
        assert_ne!(key(&o3, ""), key(&o3, "-O0"));
        let jobs = CmakeBuildOpts {
            build_jobs: Some(2),
            ..o3.clone()
        };
        assert_eq!(key(&o3, ""), key(&jobs, ""));
    }

    #[test]
    fn build_jobs_prefer_explicit_then_environment_then_host() {
        assert_eq!(resolve_build_jobs(Some(3), Some("8"), Some(16)), 3);
        assert_eq!(resolve_build_jobs(None, Some("8"), Some(16)), 8);
        assert_eq!(resolve_build_jobs(None, Some(" 4 "), Some(16)), 4);
        assert_eq!(resolve_build_jobs(None, None, Some(16)), 16);
        assert_eq!(resolve_build_jobs(None, None, None), 1);
        for invalid in ["", "0", "-2", "many", "1.5", "+", "99999999999999999999999"] {
            assert_eq!(
                resolve_build_jobs(None, Some(invalid), Some(12)),
                12,
                "environment value {invalid:?}"
            );
        }
        assert_eq!(resolve_build_jobs(Some(0), Some("5"), Some(12)), 5);
        assert_eq!(resolve_build_jobs(Some(0), None, Some(0)), 1);
    }

    #[test]
    fn build_commands_pass_parallel_and_target() {
        let args = |command: &Command| {
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        let model = build_command("cmake", Path::new("out/build"), 6, None);
        assert_eq!(
            args(&model),
            [
                "--build",
                "out/build",
                "--config",
                "Release",
                "--parallel",
                "6"
            ]
        );
        let runtime = build_command("cmake", Path::new("rt/build"), 6, Some("llg_runtime"));
        assert_eq!(
            args(&runtime),
            [
                "--build",
                "rt/build",
                "--config",
                "Release",
                "--parallel",
                "6",
                "--target",
                "llg_runtime"
            ]
        );
    }

    #[test]
    fn model_metadata_requires_current_ownership_abi() {
        assert!(validate_model_abi(&[]).is_ok());
        let config = super::super::value_backend::ValueConfig::default();
        let source = format!(
            "#define LLG_MODEL_VALUE_ABI {}\n#define LLG_MODEL_PROCESS_ABI 3\n\
             #define LLG_MODEL_VALUE_BACKEND {}\n#define LLG_MODEL_COMPACT_KERNELS {}\n",
            config.backend.abi(),
            config.backend.selector(),
            config.kernel.selector()
        );
        assert!(validate_model_abi(&[("model.c", &source)]).is_ok());
        for invalid in [
            "",
            "#define LLG_MODEL_VALUE_ABI 0\n",
            "#define LLG_MODEL_VALUE_ABI 2\n",
        ] {
            assert!(matches!(
                validate_model_abi(&[("model.c", invalid)]),
                Err(BuildError::InvalidModelAbi(_))
            ));
        }
        let duplicate = format!("{source}{source}");
        assert!(validate_model_abi(&[("model.c", &duplicate)]).is_err());
        assert!(!CMAKELISTS_TEMPLATE.contains("MODEL_WIDTH"));
        assert!(!RUNTIME_CMAKELISTS_TEMPLATE.contains("MODEL_WIDTH"));
    }

    #[test]
    fn launcher_precedence_is_option_then_environment_then_none() {
        assert_eq!(launcher_from(None, None), "");
        assert_eq!(launcher_from(None, Some("ccache")), "ccache");
        assert_eq!(launcher_from(None, Some("  ")), "");
        assert_eq!(launcher_from(Some("sccache"), Some("ccache")), "sccache");
        assert_eq!(launcher_from(Some(""), Some("ccache")), "");
        assert_eq!(launcher_from(Some("sccache"), None), "sccache");
    }

    #[test]
    fn generator_precedence_is_option_then_environment_then_ninja() {
        assert_eq!(
            generator_from(Some("Unix Makefiles"), Some("Ninja Multi-Config")),
            "Unix Makefiles"
        );
        assert_eq!(
            generator_from(None, Some("Unix Makefiles")),
            "Unix Makefiles"
        );
        assert_eq!(
            generator_from(Some(""), Some("Unix Makefiles")),
            "Unix Makefiles",
            "an empty option counts as unset"
        );
        assert_eq!(generator_from(None, Some("")), DEFAULT_GENERATOR);
        assert_eq!(generator_from(None, None), "Ninja");
    }

    #[test]
    fn generated_projects_detect_the_toolchain_like_the_seed_probe() {
        // Detection runs in project(); the seed is only valid for projects
        // whose preamble (and so policy state) matches the probe's.
        for template in [CMAKELISTS_TEMPLATE, RUNTIME_CMAKELISTS_TEMPLATE] {
            let rest = template
                .strip_prefix(toolchain_seed::PROJECT_PREAMBLE)
                .expect("generated project starts with the seed preamble");
            assert!(rest.starts_with("project("), "{rest}");
            assert!(rest.lines().next().unwrap().ends_with(" C)"));
        }
    }

    #[test]
    fn every_configure_names_the_release_build_type() {
        // MSVC's platform default is Debug for single-config generators, so a
        // configure without the argument would compile models with /Od and
        // link them with /debug under Ninja.
        let toolchain = toolchain_seed::Toolchain {
            cmake: "cmake",
            generator: "Ninja",
            cc: "cl",
            flags: "",
            launcher: "",
            compiler_identity: "id",
            compiler_target: "x64",
        };
        for seed_args in [
            vec![],
            vec!["-DCMAKE_PLATFORM_INFO_INITIALIZED:INTERNAL=1".to_owned()],
        ] {
            let command = configure_command(
                Path::new("src"),
                Path::new("build"),
                &toolchain,
                &["-DLLG_RUNTIME_LIBRARY=x".to_owned()],
                &seed_args,
            );
            let args: Vec<_> = command
                .get_args()
                .map(|arg| arg.to_string_lossy())
                .collect();
            assert_eq!(
                args.iter()
                    .filter(|arg| arg.starts_with("-DCMAKE_BUILD_TYPE"))
                    .count(),
                1,
                "{args:?}"
            );
            assert!(
                args.iter().any(|arg| arg == "-DCMAKE_BUILD_TYPE=Release"),
                "{args:?}"
            );
        }
        assert_eq!(
            build_command("cmake", Path::new("b"), 1, None)
                .get_args()
                .nth(3)
                .unwrap(),
            "Release"
        );
    }

    #[test]
    fn missing_build_program_is_reported_without_a_retry_hint() {
        #[cfg(unix)]
        use std::os::unix::process::ExitStatusExt;
        #[cfg(windows)]
        use std::os::windows::process::ExitStatusExt;
        // CMake 3.16-3.31 wording when `-G Ninja` finds no `ninja`.
        let stderr = b"CMake Error: CMake was unable to find a build program corresponding to \"Ninja\".  CMAKE_MAKE_PROGRAM is not set.  You probably need to select a different build tool.\nCMake Error: CMAKE_C_COMPILER not set, after EnableLanguage\n-- Configuring incomplete, errors occurred!\n";
        let missing = std::process::Output {
            status: std::process::ExitStatus::from_raw(1),
            stdout: Vec::new(),
            stderr: stderr.to_vec(),
        };
        assert!(missing_build_program(&missing));
        let command = Command::new("cmake");
        let error = configure_error(&command, "Ninja", &missing);
        assert!(matches!(error, BuildError::BuildProgramNotFound { .. }));
        let message = error.to_string();
        assert!(message.contains("install Ninja"), "{message}");
        assert!(message.contains("CMAKE_GENERATOR"), "{message}");
        assert!(message.contains("--generator"), "{message}");

        let other = std::process::Output {
            stderr: b"CMake Error at CMakeLists.txt:1 (project): bad\n".to_vec(),
            ..missing
        };
        assert!(!missing_build_program(&other));
        assert!(matches!(
            configure_error(&command, "Ninja", &other),
            BuildError::Configure { .. }
        ));
    }

    #[test]
    fn runtime_cache_key_varies_with_toolchain_and_waveforms_not_model_width() {
        let defaults = CmakeBuildOpts::default();
        let base = runtime_cache_key(false, "cc", "-O2", "cmake", &defaults);
        assert_eq!(
            base,
            runtime_cache_key(false, "cc", "-O2", "cmake", &defaults)
        );
        assert!(base.starts_with("owned-v"));
        assert_ne!(
            base,
            runtime_cache_key(true, "cc", "-O2", "cmake", &defaults)
        );
        let generator = |name: &str| CmakeBuildOpts {
            generator: Some(name.to_owned()),
            ..Default::default()
        };
        assert_ne!(
            runtime_cache_key(false, "cc", "-O2", "cmake", &generator("Ninja")),
            runtime_cache_key(false, "cc", "-O2", "cmake", &generator("Unix Makefiles")),
            "the effective generator selects distinct runtime archives"
        );
        let launched = CmakeBuildOpts {
            launcher: Some("ccache".to_owned()),
            ..Default::default()
        };
        assert_ne!(
            base,
            runtime_cache_key(false, "cc", "-O2", "cmake", &launched)
        );
        assert_ne!(
            runtime_cache_key_with_compiler(
                false,
                "cc",
                "-O2",
                "cmake",
                &defaults,
                "same compiler",
                "x86_64-pc-linux-gnu",
            ),
            runtime_cache_key_with_compiler(
                false,
                "cc",
                "-O2",
                "cmake",
                &defaults,
                "same compiler",
                "aarch64-pc-linux-gnu",
            ),
            "compiler-reported targets must select distinct runtime archives"
        );
    }

    #[test]
    fn runtime_cache_key_fences_the_value_ownership_abi() {
        let defaults = CmakeBuildOpts::default();
        let key = runtime_cache_key(false, "cc", "-O2", "cmake", &defaults);
        assert!(
            key.starts_with(&format!(
                "owned-v{}-wave0-",
                defaults.value_config.backend.abi()
            )),
            "runtime cache key must carry the ownership ABI: {key}"
        );
        // Generated models declaring any other ABI are rejected, so an ABI
        // bump cannot silently reuse an incompatible cached archive.
        let abi = defaults.value_config.backend.abi();
        for version in [abi.wrapping_sub(1), abi + 1] {
            let stale = format!("#define LLG_MODEL_VALUE_ABI {version}\n");
            assert!(matches!(
                validate_model_abi(&[("model.c", &stale)]),
                Err(BuildError::InvalidModelAbi(_))
            ));
        }
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
    fn sim_executable_lookup_uses_the_host_executable_suffix() {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let bin =
            std::env::temp_dir().join(format!("llg-find-sim-exe-{}-{unique}", std::process::id()));
        let name = format!("sim{}", std::env::consts::EXE_SUFFIX);
        // Multi-config generators place the executable in a configuration
        // directory; a same-stem file with another extension is not it.
        std::fs::create_dir_all(bin.join("Release")).unwrap();
        std::fs::write(bin.join("Release").join(&name), b"").unwrap();
        std::fs::write(bin.join("sim.pdb"), b"").unwrap();
        assert_eq!(find_sim_exe(&bin).unwrap(), bin.join("Release").join(&name));

        std::fs::write(bin.join(&name), b"").unwrap();
        assert_eq!(find_sim_exe(&bin).unwrap(), bin.join(&name));

        std::fs::remove_dir_all(&bin).unwrap();
        assert!(matches!(
            find_sim_exe(&bin),
            Err(BuildError::ExecutableNotFound { .. })
        ));
    }

    #[test]
    fn explicit_tool_options_override_the_environment() {
        let opts = CmakeBuildOpts {
            cc: Some("explicit-cc".to_owned()),
            cflags: Some("-g  -DX=1".to_owned()),
            cmake: Some("explicit-cmake".to_owned()),
            ..Default::default()
        };
        assert_eq!(resolve_cc(&opts), "explicit-cc");
        assert_eq!(
            select_cc(Some("explicit-cc"), Some("llg-cc"), Some("cc-env")),
            "explicit-cc"
        );
        assert_eq!(select_cc(None, Some("llg-cc"), Some("cc-env")), "llg-cc");
        assert_eq!(select_cc(None, Some(""), Some("cc-env")), "cc-env");
        assert_eq!(select_cc(None, None, Some("")), DEFAULT_C_COMPILER);
        assert_eq!(select_cc(None, None, None), DEFAULT_C_COMPILER);
        assert_eq!(DEFAULT_C_COMPILER, if cfg!(windows) { "cl" } else { "cc" });
        assert_eq!(resolve_cmake(&opts), "explicit-cmake");
        let flags = c_flags(&opts).unwrap();
        assert_eq!(flags, "-g -DX=1");

        let quoted = CmakeBuildOpts {
            cflags: Some("-DX=\"y\"".to_owned()),
            ..Default::default()
        };
        assert!(matches!(
            c_flags(&quoted),
            Err(BuildError::InvalidCompilerFlag(_))
        ));
    }

    #[test]
    fn runtime_cache_defaults_to_base_build_and_accepts_override() {
        let base = std::env::temp_dir().join("llg-runtime-cache-base");
        let default = base.join("build/llg-runtime-cache");
        assert_eq!(runtime_cache_root_with_override(&base, None), default);
        assert_eq!(
            runtime_cache_root_with_override(&base, Some(PathBuf::new())),
            default,
            "an empty override must not place the cache in the base itself"
        );

        let relative_override = PathBuf::from("custom/runtime-cache");
        assert_eq!(
            runtime_cache_root_with_override(&base, Some(relative_override.clone())),
            base.join(relative_override)
        );

        let absolute_override = std::env::temp_dir().join("llg-custom-runtime-cache");
        assert_eq!(
            runtime_cache_root_with_override(&base, Some(absolute_override.clone())),
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
            let cmake = std::fs::read_to_string(dir.join("CMakeLists.txt"))
                .map_err(|e| format!("read waveform CMakeLists.txt: {e}"))?;
            if cmake.contains("find_package(ZLIB") || !dir.join("zlib/zlib.h").is_file() {
                return Err("waveform models must build the bundled zlib".to_string());
            }
            // Hosts with system zlib headers would otherwise mask a missing
            // bundled include path for model sources that include fstapi.h.
            if !cmake.contains("target_include_directories(sim PRIVATE ${CMAKE_SOURCE_DIR}/zlib)")
                || !cmake.contains("target_compile_definitions(sim PRIVATE Z_PREFIX)")
            {
                return Err("waveform model sources must use the bundled zlib header".to_string());
            }
            generate_model_sources(&dir, &[("plain.c", "int main(void) { return 0; }\n")])
                .map_err(|error| error.to_string())?;
            if dir.join("llg_wave.c").exists() || dir.join("zlib").exists() {
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
