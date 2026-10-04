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
  containing `/`.
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
  function as unreachable (C4702); GCC/Clang do not. `llg_co.h` omits such
  defensive returns only for MSVC with its default noreturn handler.
- Without zlib's configure step `Z_HAVE_UNISTD_H` is unset, so its `gz*` code
  calls undeclared `read`/`write`/`lseek`/`close`, which GCC 14 and Clang reject;
  the bundled zlib defines it outside Windows.
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
