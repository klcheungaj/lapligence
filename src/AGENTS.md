# Shared source policies

`lib.rs` exposes `ffi`, `core`, `sim` and `memory_limit`. Root architecture rules
apply throughout; child guides own module contracts.

## Process-memory safeguard

[memory_limit.rs](memory_limit.rs) owns policy;
[ffi/process_memory.rs](ffi/process_memory.rs) owns platform primitives. `llg` and
`llg_ls` install the guard after accepting CLI work and before source admission,
not for help, version or usage errors. `llg_ls` integrates lifecycle logging;
`llg` sends status/warnings to stderr.

The optional process-wide budget covers Rust and in-process Slang/C++ allocations.
Configure it before starting either frontend:

| Variable | Meaning | Default |
| --- | --- | --- |
| `LLG_MEMORY_LIMIT_MB` | Positive integer physical-memory budget in MiB | unset/disabled |
| `LLG_MEMORY_WARNING_PERCENT` | Percentage triggering one warning | `80` |
| `LLG_MEMORY_POLL_MS` | Sampling interval in milliseconds | `1000` |
| `LLG_MEMORY_ADDRESS_SPACE_LIMIT` | Reversible Linux/macOS `RLIMIT_AS`; accepts `true/false`, `yes/no`, `on/off`, `1/0` | `false` |

The watchdog samples Linux RSS, macOS physical footprint/resident usage or Windows
working set. Budget exhaustion prints a fixed emergency stderr message and
terminates immediately, avoiding shutdown allocations. Windows also uses a Job
Object process limit. Optional Unix `RLIMIT_AS` limits virtual, not physical,
memory and may fail allocations before the watchdog. Failed native installation
is reported; retain the watchdog when it can start, including inside an existing
Windows host job.

Budget for startup and peak Slang parsing/capture, not idle usage. The guard covers
concurrent LSP work, frontend compilations and driver activity. It samples and
terminates only the frontend, not its generated simulator or aggregate child RSS.
Unix children inherit enabled `RLIMIT_AS`; Windows descendants remaining in the job
may inherit native limits. These effects are host-dependent; guard child physical
memory separately.

## Safeguard changes

[LSP admission](bin/llg_ls/lsp/AGENTS.md) owns staging;
[LSP policy](bin/llg_ls/AGENTS.md) owns requests, caches, explorer limits and tracing;
[validation](../tests/AGENTS.md) owns gates.

- Apply byte/count limits before input cloning, size-derived allocation, cache
  keys, single-flight admission, staging, parsing or optional-response serialization.
  Preserve exact boundaries; check or saturate arithmetic for totals, percentages,
  timeouts and size conversions.
- Keep open buffers authoritative. Bound closed reads to maximum plus one byte
  and reuse admitted snapshots. Preserve include authorization, canonical
  deduplication and cache-only native reads; never reopen unadmitted paths.
- Reject invalid configuration atomically, retaining the documented last-valid
  config and servable analysis. Stale revisions or saturated flights must not
  start frontend work.
- Keep emergency logs allocation-light, bounded, redacted and off JSON-RPC stdout.
  Test Unix limit restoration and Windows Job Object ownership natively before
  changing platform claims.
- Update defaults, diagnostics, tests and owning guides together for any changed
  environment variable, configuration key, limit or fallback.

## Platform layers

Platform conditionals (`cfg(windows|unix)`, `target_os`, `_WIN32`, `_MSC_VER`,
`__GLIBC__` and similar) live only in the platform layers; everything else calls
their neutral interface:

- Rust: `src/ffi/platform.rs` (path spellings such as verbatim-prefix removal
  and `canonicalize`, file-name rules), `ffi/secure_fs.rs` (handle-based file
  admission) and `ffi/process_memory.rs` (process limits).
- Build scripts: `build_support/host_platform.rs`, included beside the other
  build-support modules because build scripts cannot use the crate.
- Generated-model C runtime: `src/sim/rt/llg_compiler.h`, `llg_platform.h` and
  `llg_platform_native.h` (see [runtime](sim/rt/AGENTS.md#platform-layer)).
- C++ wrapper: `src/wrapper/slang_platform.hpp/.cpp`.

Tests may branch on the platform only where they assert platform-specific
behaviour (symlinks, permissions). Add a layer function instead of a new
conditional elsewhere.

## Platform differences

Observed in CI and native builds; keep code portable across all of them. Resolve
or compare paths the way [secure_fs](ffi/secure_fs.rs) does, never by string.

- macOS `/var`, `/tmp` and `/etc` are symlinks into `/private`, and `temp_dir()`
  is `/var/folders/...`. Handle-derived paths are resolved, so compare resolved
  spellings: resolve user roots (include directories, dump targets) once at entry
  with `secure_fs::open_path(..).actual_path()`. Linux hits the same mismatch for
  symlinked project directories. Reproduce macOS by pointing `TMPDIR` at a
  symlinked directory. Never send resolved paths back to LSP clients in place of
  their URI spelling; symlinked LSP workspace roots still miss feature lookups.
- macOS `/dev/fd/N` entries are fdesc device nodes, not symlinks: `readlink` fails
  with EINVAL, `opendir` with ENOTDIR, and opening one duplicates N with a shared
  offset. Use `fcntl(F_GETPATH)` and `fdopendir` on a fresh `openat(".")`. Linux
  `/proc/self/fd/N` is a real link whose reopen creates a new description.
- Windows `canonicalize` returns verbatim `\\?\D:\...` paths; `Path::starts_with`
  treats that prefix and `D:` as different. Handle paths strip it, so compare
  canonical with canonical, and canonicalize test temporary paths on Unix only.
- Windows relative paths use `\`: compare `Path` components, never strings
  containing `/`. Diagnostics and runtime reports name a source by its resolved
  native spelling (handle-derived); tests build expected text with
  `ffi::platform::canonicalize`, not `join("tests/fixtures/...")`, which keeps
  a literal `/` on Windows. `Url::path` is `/C:/...` there; compare
  `Url::to_file_path` or `Url::from_file_path` results instead.
- Windows text-mode consoles and files write CRLF. Captured child output goes
  through `ffi::platform::native_text_to_lf` before comparison with LF oracles.
  `$fopen` passes its mode to C `fopen` unchanged, so text-mode `$ftell`,
  `$fseek` and `$rewind` positions differ there (UCRT's text-mode `ftell`
  returned -1 after `$rewind` and `$fgetc` on an LF-only file). HDL fixtures and
  `lrm_decisions` cases whose output depends on byte positions or unformatted
  data open their files with `"rb"`/`"wb"` (IEEE 1800-2009 21.3.1).
- Windows `abort()` ends a process with an NTSTATUS such as 0xC0000409, which an
  `ExitCode` byte would truncate (to 9); `llg` maps statuses outside 0-255, like
  Unix signals, to 1.
- MSVC `/W4` rejects shadowed locals, parameters and globals (C4456/C4457/C4459)
  under `/WX`; GCC/Clang only report them with `-Wshadow`, which the runtime
  storage probes enable.
- Windows share mode 0 blocks other opens of that file, including from this
  process (os error 32). `FlushFileBuffers` needs a writable handle, so directory
  sync through cap-std's read-only directory handles fails; skip it there.
- `std::os::windows::fs::MetadataExt::number_of_links`/`file_index` are unstable
  (`windows_by_handle`); use cap-std `MetadataExt` on an open handle. Gate
  `std::os::unix` users, including symlink tests, with `#[cfg(unix)]`.
- MSVC C mode lacks `max_align_t` (use `llg_co_max_align_t`). MSVC gives unnamed
  enums an `int` underlying type where GCC/Clang choose `unsigned`, so brace
  initializing a `uint32_t` from an enum ternary is narrowing (C2397); cast.
  MSVC's `<chrono>` warning C4530 without `/EHsc` is harmless.
- MSVC at `/W4` reports a statement after a call to a `__declspec(noreturn)`
  or `_Noreturn` function as unreachable (C4702); GCC/Clang do not. `llg_co.h`
  omits such defensive returns only for MSVC with its default noreturn handler
  or an override that defines `LLG_CO_BAD_STATE_NORETURN` (as `llg_rt.h` does).
- MSVC `/W4 /WX` also rejects what GCC/Clang accept by default: C4701 for a
  variable assigned only inside a `while` loop (at `/O2`; use `do`), C4702 for a
  `return` after an `abort()` helper that `/O2` inlines (declare it `_Noreturn`),
  C4090 for `memset` on an array of pointers to const (assign elements), and
  C4132 for a valid C11 tentative definition `static const T x;` completed later
  (define the object first, after a function prototype). Clang `-Wconditional-uninitialized`/`-Wunreachable-code` approximate the first
  two. Details: [runtime guide](sim/rt/AGENTS.md).
- `cl` writes its diagnostics to stdout, not stderr: print both streams when a
  compile fails. A Ninja build stops at the first failing source, so one CI run
  can hide further MSVC-only errors behind it.
- Unsequenced operands (`f() | g()`, C11 6.5p3) run in a different order under
  MSVC than under GCC/Clang. Sequence calls with side effects, such as printing
  checks, as separate statements.
- CMake's Windows-MSVC platform file makes single-config generators (Ninja,
  NMake) default to Debug: `/Od` keeps dead calls, so a subset library can fail
  to link, and `/Zi` writes PDBs. Pass `CMAKE_BUILD_TYPE` and
  `CMAKE_TRY_COMPILE_CONFIGURATION` explicitly.
- Windows paths are limited to `MAX_PATH` (260 characters). CMake `try_compile`
  object paths under a deep test directory exceed it, and `cl` fails with C1083
  or C1041, which surfaces as unrelated errors such as a failed type-size check.
  Keep scratch directory names short (`tests/readme.md`).
- CMake output differs on Windows in ways a textual comparison must allow:
  `build.ninja` escapes paths (`D$:\...`, `$ `, `$$`), configured headers are
  written with CRLF, `CMakeFiles/ShowIncludes/` is scratch, and the default
  `CMAKE_INSTALL_PREFIX` depends on whether `CMAKE_SIZEOF_VOID_P` is known yet.
  The toolchain seed handles these ([sim guide](sim/AGENTS.md)).
- Git for Windows' MSYS `make`/`sh` rewrites `/option` arguments into paths
  (`/nologo` becomes `C:/Program Files/Git/nologo`); use the NMake or Ninja
  generator with MSVC, never `Unix Makefiles`.
- Paths built from configured relative parts get native separators on Windows
  (`out\w\dump.vcd`); tests build expected paths with `Path::join`.
- `ilammy/msvc-dev-cmd` with `amd64_arm64` runs x64 cross tools under emulation
  on arm64 runners, several times slower; use the native `arm64` tools.
- Without zlib's configure step `Z_HAVE_UNISTD_H` is unset, so its `gz*` code
  calls undeclared `read`/`write`/`lseek`/`close`, which GCC 14 and Clang reject;
  the bundled zlib defines it outside Windows.
- Windows test temporary directories drop the test's label (`llg-<pid>-<n>`),
  so a word that a Unix run finds in a diagnostic's path is absent there. Match
  diagnostic fields (message, severity, position), never a whole `Debug` dump.
- The GitHub Windows arm64 image ships Defender real-time, on-access,
  behaviour and IOAV protection on, with tamper protection, so
  `Set-MpPreference` cannot turn them off; the x64 image ships them off. Both
  list `C:\` and `D:\` as excluded paths. Under the test load a fresh, seeded
  model configure took 3.0 s (median) on arm64 against 0.15 s for an existing
  tree, 0.35 s on x64 and 19 ms on Linux; `scripts/ci_windows_runner.py`
  measures where that time goes. Doing less per fresh tree is the remaining
  lever (opt-in `LLG_CMAKE_TREE_TEMPLATES`, [sim guide](sim/AGENTS.md)).
- On the arm64 runner `bash` (Git Bash) and Git's `link` are x64 images run
  under emulation, and Git's `usr/bin` precedes the MSVC tools on a Git Bash
  `PATH`. CMake still links with the native `link.exe` beside `cl`
  (`CMakeFindBinUtils` searches the compiler's directory first); Cargo needs
  the pinned linker (`.github/actions/msvc-linker`).
- The UCRT's `clock()` returns elapsed wall time, not processor time; timing
  checks that compare CPU costs use `llg_process_cpu_seconds`
  (`llg_platform_native.h`).
- Renaming `malloc`/`free` with `-D` on the compiler command line also renames
  the UCRT's own `dllimport` declarations, so references become `__imp_<name>`
  that no static library defines (LNK2019). Rename in a header included after
  `<stdlib.h>` (a forced include).
- ccache refuses MSVC `/Zi` (shared PDB) as an unsupported compiler option;
  Debug CMake projects that should be cached embed debug information
  (`/Z7`, `CMAKE_MSVC_DEBUG_INFORMATION_FORMAT=Embedded`, CMP0141).
- Linux CI builds only in static-musl Alpine; glibc Ubuntu/Rocky containers run
  those binaries. Minimal images lack clang, which `generated_c_frame_lint`
  requires alongside gcc; install test tools explicitly. zlib is bundled, never
  a system package.

## Pipeline profiling

`profile.rs` emits opt-in `llg-profile begin/end` stderr markers when
`LLG_PROFILE_STAGES=1`. Wrapper markers separate Slang elaboration/analysis from
capture; FFI, DB import, semantic/typed/execution lowering, optimization and
rendering use the same format. Keep this diagnostic helper frontend-neutral;
markers must not affect ABI fields, diagnostics or generated files.
`perf/scripts/frontend_scale.py` combines these markers with sampled Linux RSS
and GNU time whole-process peaks. RSS includes all representations live in a
stage and allocator-retained pages, not that stage's exclusive allocation cost.
