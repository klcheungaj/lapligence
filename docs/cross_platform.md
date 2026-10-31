# Cross-platform handling

Platform behaviour observed in CI and native builds that lapligence code, tests
and workflows must stay portable across. Each entry names the pitfall and the
code that handles it; the code comments hold the details.

Release targets: static-musl Linux x86_64/arm64, MSVC Windows x86_64/arm64 and
macOS arm64. Windows on ARM ships `llg_ls` only (see [arm64](#arm64)). A
configured matrix leg is not validated support.

## Platform layers

Platform conditionals (`cfg(windows|unix)`, `target_os`, `_WIN32`, `_MSC_VER`,
`__GLIBC__` and similar) live only in these layers; other code calls their
neutral interface:

| Code | Layer |
| --- | --- |
| Rust library | `src/ffi/platform.rs` (path spellings, file-name rules), `src/ffi/secure_fs.rs` (handle-based admission), `src/ffi/process_memory.rs` (process limits) |
| Build scripts | `build_support/host_platform.rs` (build scripts cannot use the crate) |
| Generated-model C runtime | `src/sim/rt/llg_compiler.h`, `llg_platform.h`, `llg_platform_native.h` |
| C++ wrapper | `src/wrapper/slang_platform.hpp/.cpp` |

- Tests branch on the platform only where they assert platform-specific
  behaviour (symlinks, permissions).
- Add a layer function instead of a new conditional elsewhere.

## Paths and text

- Resolve and compare paths the way `ffi/secure_fs.rs` does, never by string.
  Compare resolved with resolved: resolve user roots (include directories, dump
  targets) once at entry with `secure_fs::open_path(..).actual_path()`.
- Never send resolved paths back to LSP clients in place of their URI spelling;
  symlinked LSP workspace roots still miss feature lookups.
- Diagnostics and runtime reports name a source by its resolved native spelling.
  Tests build expected text with `sim_harness::source_display`, not
  `join("tests/fixtures/...")`, which keeps a literal `/` on Windows.
- Compare `Path` components, never strings containing `/`; configured relative
  parts get native separators (`out\w\dump.vcd`).
- Text read back from disk or a child process can hold CRLF on Windows, whatever
  the test wrote. Sources: text-mode C runtime writes (console and `stdout` of
  models and companions, `$fopen` with `"w"`, `$fwrite`/`$fdisplay`, `printf`),
  Git writing a working tree (`checkout`, `restore`, `clone`, `stash`: Windows
  images set `core.autocrlf=true` system-wide, and the repository's
  `.gitattributes` `eol=lf` covers only this checkout, never a repository a test
  creates), patch tools and generated or configured files. Compare such text with
  an LF oracle only after `ffi::platform::native_text_to_lf`
  (`sim_harness::host_text_to_lf`), or make the bytes fixed at the source:
  binary-mode opens (`"rb"`/`"wb"`) or `core.autocrlf=false` in a repository the
  test creates (`build_support/vendor_patches.rs` tests). Code that rewrites a
  file keeps that file's newline convention (`vendor_patches` preserves a CRLF
  checkout). [Linux guards](../tests/readme.md#msvc-and-windows-guards-on-linux).
- Match diagnostic fields (message, severity, position), never a whole `Debug`
  dump: Windows test temporary directories drop the test label (`llg-<pid>-<n>`).

## Windows (MSVC)

Most Windows-only failures are MSVC `/W4 /WX` diagnostics, path forms (drive
letters, separators, verbatim prefixes) and DLL linkage. Each new one gets a
bullet here and, where possible, a Linux-side guard test.

Paths and files:

- `canonicalize` returns verbatim `\\?\D:\...` paths, which `Path::starts_with`
  treats as different from `D:`; handle paths strip the prefix. Canonicalize test
  temporary paths on Unix only.
- `Url::path` is `/C:/...`; compare `Url::to_file_path`/`from_file_path` results.
- `MAX_PATH` (260) breaks deep CMake `try_compile` object paths (`cl` C1083/C1041,
  surfacing as unrelated errors such as a failed type-size check). Keep scratch
  names short ([test storage](../tests/readme.md#test-build-storage)).
- Share mode 0 blocks other opens of the file, even from this process (os error
  32). `FlushFileBuffers` needs a writable handle, so directory sync through
  cap-std's read-only directory handles is skipped.
- `std::os::windows::fs::MetadataExt::number_of_links`/`file_index` are unstable;
  use cap-std `MetadataExt` on an open handle. Gate `std::os::unix` users,
  including symlink tests, with `#[cfg(unix)]`.
- A CLI operand of the form `prefix:path` (`--sdf [min:|typ:|max:]file`) must not
  read a drive letter `D:` or a UNC/`\\?\` path as the prefix: prefix words have
  two or more letters (`sdf_operand_*` unit tests in
  `src/sim/codegen/lowering/collection/sdf_annotations.rs`).
- Expected diagnostics in tests name the source as llg prints it, with native
  separators (`sim_harness::source_display`); a `/`-joined fixture path matches
  on Linux and macOS and cannot fail there.
- `$fopen` passes its mode to C `fopen` unchanged, so text-mode (CRLF, see
  [paths and text](#paths-and-text)) `$ftell`/`$fseek`/`$rewind` positions
  differ; fixtures and `lrm_decisions` cases that depend on byte positions or
  unformatted data open files with `"rb"`/`"wb"` (IEEE 1800-2009 21.3.1).

Processes:

- `abort()` ends a process with an NTSTATUS such as 0xC0000409 that an `ExitCode`
  byte would truncate; `llg` maps it to 1 with one status line (`model_exit` in
  `src/bin/llg/main.rs`).
- Process CPU time (`GetProcessTimes`) advances in 15.625 ms ticks and the UCRT
  `clock()` returns wall time. Use `llg_process_cpu_seconds`
  (`llg_platform_native.h`) and make CPU-time comparisons span many ticks.
- `cmd.exe`'s `copy` prints `1 file(s) copied.` on the model's stdout from
  `$system`; redirect it (`>NUL`) where output is compared.

MSVC compiler:

- `/W4 /WX` rejects what GCC/Clang accept by default: shadowed locals,
  parameters and globals (C4456/C4457/C4459; the runtime storage probes enable
  `-Wshadow` elsewhere), a variable assigned only inside a `while` loop at `/O2`
  (C4701; use `do`), a statement after a noreturn call or an inlined `abort()`
  helper (C4702; declare it `_Noreturn`; `llg_co.h` documents its
  `LLG_CO_BAD_STATE_NORETURN` case), `memset` on an array of pointers to const
  (C4090; assign elements) and a completed tentative definition
  `static const T x;` (C4132; define the object first). A local set only by an
  out-parameter call that a short-circuit can skip is C4701 even when a flag
  guards its use; initialize it. Clang
  `-Wconditional-uninitialized`/`-Wunreachable-code` approximate C4701/C4702.
  A cast that truncates a constant is C4310 (GCC and Clang never warn on an
  explicit cast): a complemented constant, `(uint8_t)~FLAG` (mask first,
  `(uint8_t)(x & ~FLAG)`), or a literal the type cannot hold, `(PLI_BYTE8)0xff`
  (plain `char` is signed; `memset` an all-ones byte)
  (`runtime_strict_sources::runtime_sources_have_no_truncating_constant_casts`).
- Implicit narrowing is C4244 (`int` to `int8_t`, `double` to `float` or an
  integer) and C4267 (`size_t` to a 32-bit type), errors under `/WX`; convert
  explicitly after checking the range, e.g. a 0/1 flag as `(int8_t)(flag != 0)`
  (`runtime_strict_sources::runtime_and_companion_c_has_no_narrowing_or_uninitialized_use`,
  which also checks C4701).
- The UCRT deprecates portable C11 routines (`sprintf`, `fopen`, `getenv`,
  `strcpy`, ...) in favour of `_s` variants (C4996). Every strict harness build
  (`tests/support/c_compiler.rs`, `runtime_value_storage` CMake) defines
  `_CRT_SECURE_NO_WARNINGS`; a new `/W4 /WX` build site needs it too. Fixtures
  still prefer bounded calls (`snprintf`).
- C mode lacks `max_align_t` (use `llg_co_max_align_t`). Unnamed enums have an
  `int` underlying type, so brace-initializing a `uint32_t` from an enum ternary
  narrows (C2397); cast. `<chrono>` warning C4530 without `/EHsc` is harmless.
- Unsequenced operands (`f() | g()`, C11 6.5p3) run in another order than under
  GCC/Clang; sequence calls with side effects as separate statements.
- `cl` writes diagnostics to stdout: print both streams on failure. Ninja stops
  at the first failing source, so one CI run can hide further MSVC-only errors.
- Renaming `malloc`/`free` with `-D` also renames the UCRT's `dllimport`
  declarations (LNK2019 on `__imp_<name>`); rename in a forced include after
  `<stdlib.h>`.
- ccache refuses `/Zi`; cached Debug projects embed debug information (`/Z7`,
  `CMAKE_MSVC_DEBUG_INFORMATION_FORMAT=Embedded`, CMP0141).
- Debug (`/Od /RTC1`) runs call-heavy probes 6-14x slower than GCC `-O0`; keep
  per-bit reference work in plain arrays, not calls into the runtime under test.

CMake:

- The Windows-MSVC platform file makes single-config generators default to Debug
  (`/Od` keeps dead calls, `/Zi` writes PDBs): pass `CMAKE_BUILD_TYPE` and
  `CMAKE_TRY_COMPILE_CONFIGURATION` (`src/sim/build/toolchain_seed.rs`).
- Textual comparisons of configured trees allow `build.ninja` escapes, CRLF
  configured headers, `CMakeFiles/ShowIncludes/` scratch, an install prefix that
  depends on `CMAKE_SIZEOF_VOID_P`, and a `CMakeCache.txt` header naming the
  build directory with a lower-case drive and `/` (`toolchain_seed.rs`,
  `tree_template.rs`, `sim_cmake::cmake_path_spellings`).
- Git for Windows' MSYS `make`/`sh` rewrites `/option` arguments into paths; use
  the NMake or Ninja generator with MSVC, never `Unix Makefiles`.

DPI-C and VPI DLLs:

- A DLL resolves every import at link time, so companions reach `svdpi.h`, `vpi_*`
  and DPI-C exports through `sim.exe`'s import library (`--dpi-import-lib`); the
  design and naming rules are in the `src/sim/build/import_library.rs` module
  docs. Static archives remain the route that needs no import library
  (`sim_041::static_archive_companion_reaches_exports_and_scopes`).
- User C stays Annex H/I code: mark every import definition `DPI_DLLESPEC` (or use
  a `.def`), and its prototype too, or MSVC reports C2375 "redefinition; different
  linkage". A DLL that exports nothing gets no import library, and `--dpi-lib
  x_dll.lib` then fails as "not a regular file"
  (`sim_040::dll_companions_export_every_import_definition`). `vpi_user.h`
  exports `vlog_startup_routines` (`PLI_DLLESPEC`, Annex K).
- Foreign call contexts of suspendable DPI-C chains (ADV-018,
  `src/sim/rt/foreign_contexts/readme.md`) are platform-layer threads with an
  explicit stack (`llg_thread_start_sized`): `CreateThread` reserves
  `LLG_DPI_CONTEXT_STACK_BYTES` with `STACK_SIZE_PARAM_IS_A_RESERVATION`;
  POSIX sets it with `pthread_attr_setstacksize`, because new threads get
  only 128 KiB on musl and 512 KiB on macOS. Only models with such chains
  link `Threads::Threads` (glibc before 2.34 needs `-lpthread`).
- `cl /LD` implies the static CRT, whose `stdout` buffer the model cannot flush;
  test DLLs build with `/MD` (`tests/support/c_compiler.rs`), and
  [known issues](known_issues.md) tells users the same.

## macOS

- `/var`, `/tmp` and `/etc` are symlinks into `/private`, and `temp_dir()` is
  `/var/folders/...`; handle-derived paths are resolved (see
  [paths](#paths-and-text)). Linux hits the same mismatch for symlinked project
  directories; reproduce by pointing `TMPDIR` at a symlinked directory.
  In-memory sources (the language server's admitted buffers) keep their
  caller's spelling, so `core::compile::compile` hands Slang both spellings of
  each include directory (`slang_include_dirs`); with the resolved one alone,
  every `` `include`` under a `/var/...` or Windows `RUNNER~1` short-name root
  failed to open in `llg_ls`.
- `/bin/bash` is 3.2: under `set -u`, `"${empty[@]}"` is an unbound-variable
  error (bash 4.4+ accepts it), so a CI script tested only on Linux can stop
  on macOS; expand optional arrays as `${a[@]+"${a[@]}"}`
  (`scripts/llg-co-compile-gate.sh`).
- `/dev/fd/N` entries are fdesc device nodes, not symlinks; `secure_fs.rs`
  documents the `F_GETPATH`/`fdopendir` handling. Linux `/proc/self/fd/N` is a
  real link whose reopen creates a new description.
- The Annex I sample `svdpi.h` falls back to `<sys/types.h>`, which lacks the
  fixed-width types on macOS; llg writes its own `rt/svdpi.h` with
  `<inttypes.h>`. `-U__linux` on Linux reproduces the failure.
- Apple Clang builds the C fixtures (DPI-C/VPI companions) at `-Wall -Wextra
  -Werror`, and Clang's `-Wall` has warnings GCC's lacks, such as
  `-Wunused-but-set-variable` for a counter that is only incremented
  (`adv_019/threads.c`); `runtime_strict_sources::companion_c_builds_under_clang_wall_wextra`
  runs Linux Clang over every fixture with those flags.
- Apple ld64 does not check a dylib's undefined symbols; llg reads Mach-O symbol
  tables before the build (`src/sim/build/dpi_link.rs` module docs).
  `clang --target=arm64-apple-macos11 -c` makes Mach-O objects on Linux for
  testing that reader.

## Linux musl and glibc

- Linux CI builds only in static-musl Alpine; glibc Ubuntu/Rocky containers run
  those binaries. Minimal images lack clang, which `generated_c_frame_lint`
  needs beside gcc; install test tools explicitly.
- The Ubuntu/Rocky test containers are minimal: no zlib or other `-dev`
  headers. A test that compiles C including `fstapi.h` passes `vendor/zlib`
  and `-DZ_PREFIX` as model builds do; a host with `/usr/include/zlib.h`
  hides the omission.
- zlib is bundled, never a system package. Without its configure step
  `Z_HAVE_UNISTD_H` is unset and GCC 14/Clang reject its `gz*` code; the bundled
  build defines it outside Windows.
- The build container runs as root and Cargo extracts crates with their archive
  modes (some owner-only), so the workflow hands the registry back to the runner
  user before the cache save.
- On musl, root `build.rs` and `mimalloc_shim.c` wrap C `malloc`/`free` for every
  binary ([executables](../src/bin/readme.md)).
- Compiler versions: GCC 11 (Rocky Linux 9, Ubuntu 22.04 lint job) reports
  `-Wmaybe-uninitialized` at `-O2`/`-O3` for a local filled through an inlined
  out-parameter that the callee writes only on success, even when the caller
  reads it only after success; GCC 13+ jump threading removes that path and
  stays silent (CI: `llg_vpi_dynamic.c:2061 'ai'`). Initialize such locals at
  their declaration, and let helpers write their out-parameter on every path
  (`decode_handle`). `-O3 -fno-thread-jumps`
  (`c_compiler::GCC11_MAYBE_UNINITIALIZED_FLAGS`, used by
  `runtime_strict_sources::runtime_units_have_no_gcc11_maybe_uninitialized_use`
  and `runtime_value_storage::release_runtime_units_build_under_strict_warnings`)
  brings those findings back on newer GCC, but not equally: under it GCC 13
  (Ubuntu 24.04 CI) reported two dozen sites that GCC 14 accepts. A GCC 14
  host does not reproduce CI; check runtime C that adds such locals with
  GCC 13 (`LLG_CC=gcc-13`) when available.

## arm64

AArch64 (Linux arm64, macOS arm64, Windows on ARM) and x86_64 differ in ways C
and Rust code can observe. The sanitizer job runs on Linux x86_64 only, so
arm64-only undefined or unspecified behaviour reaches CI as a plain output
mismatch; items name an x86_64 reproduction where one exists.

### Lanes and runners

- Windows on ARM (`aarch64-pc-windows-msvc`): `llg_ls` only. `llg`, generated
  models, DPI/VPI and the C runtime are unsupported;
  `ffi::platform::simulator_unsupported_reason` makes `llg` exit 2. The library,
  frontend, Db and lint code used by `llg_ls` must stay portable there. CI lane:
  [Windows on ARM](../tests/ci.md#windows-on-arm).
- `linux-arm64` builds `aarch64-unknown-linux-musl` on `ubuntu-24.04-arm` and
  runs the suite on Ubuntu 24.04 and Rocky Linux 9 with GCC-built models;
  `macos-arm64` runs on `macos-15` with Apple Clang models
  ([triggers and pipelines](../tests/ci.md#triggers-and-pipelines)). Runner
  speed varies between runs (Rocky arm64 has been the slowest Linux lane; in run
  110367163025 it was the fastest at 2944 s against 4157-4480 s on x86_64), so
  no test may depend on timing.
- tower-lsp 0.20 runs notifications concurrently, and slower runners interleave
  handlers; state written after an `await` must not let an older result replace
  a newer one (`DiscoveryOrder` in `src/bin/llg_ls/lsp/handlers/state.rs`).
- Release executables are native: `scripts/verify-macos-release.sh` rejects any
  binary that is not a Mach-O arm64 executable, so an x86_64 build running under
  Rosetta 2 (with x86_64 semantics for everything below) cannot ship.
- GitHub Windows arm64 runners: use the native `arm64` MSVC tools, not
  `amd64_arm64` (emulated x64 cross tools). Git Bash and Git's `link` are
  emulated x64 and Git's `usr/bin` precedes the MSVC tools on `PATH`; CMake still
  finds the native `link.exe` beside `cl`, and Cargo needs the pinned linker
  (`.github/actions/msvc-linker`).
- Kept for a possible return of the simulator to Windows on ARM: that image
  ships Defender real-time protection on, with tamper protection (the x64 image
  ships it off). A fresh seeded model configure took 3.0 s median there against
  0.35 s on x64 and 19 ms on Linux, and model-building tests ran a median 4.5x
  longer than on x64 (cause unknown) while compile-only tests ran at x64 speed.
  `scripts/ci_windows_runner.py` measures it; opt-in `LLG_CMAKE_TREE_TEMPLATES`
  reduces per-tree work.

### C language and ABI

- Plain C `char` is unsigned on AArch64 Linux (also PowerPC, RISC-V, MSVC `/J`)
  and signed on x86, Apple arm64 and Windows. SV `byte` is C `char` (Table H.1):
  DPI thunks keep `char` prototypes and sign-extend each foreign byte, and C code
  (companions included) reads a `byte` through an explicit sign extension, never
  `%d` of a plain `char`. `-funsigned-char` reproduces AArch64 on x86_64.
- The order in which a call's arguments (or an operator's operands) are
  evaluated is unspecified, and it splits by target: GCC on x86_64 evaluates
  arguments right to left, GCC on AArch64 and Clang on every target left to
  right. Never put two calls whose side effects interact in one argument list.
  VPI is the usual case: every VPI call but `vpi_chk_error` resets the error
  status (IEEE 1800-2009 38.2), so `vpi_printf("%s %d", vpi_get_str(...),
  vpi_chk_error(NULL))` printed level 3 on x86_64 and 0 on every arm64 lane
  (A19-D7, A19-D8); read the status into a variable first.
  `tests/vpi_application_sources.rs` rejects that shape in test C sources, and
  `LLG_CC=clang` reproduces the arm64 order on x86_64.
- `long double` is the 80-bit x87 format on x86_64 Linux, software IEEE
  binary128 on Linux arm64 (slow) and plain `double` on Apple arm64 and every
  MSVC target. Never rely on its extra precision for an expected output: the VPI
  time conversions in `src/sim/rt/llg_vpi.c` that widen through it agree across
  targets only while the product stays within 2^53.
- Apple's arm64 ABI passes every variadic argument on the stack in its own
  8-byte slot, while AAPCS64 (Linux) and x86_64 pass them in registers like named
  arguments. Call a variadic runtime entry (`llg_display`/`llg_write`, the
  `svGet*ArrElem*` index lists, `vpi_printf`, `vpi_control`) only through its
  prototype, never through a non-variadic function pointer or an undeclared
  name, and pass each argument with the exact type `va_arg` reads (an `int`
  where a 64-bit value is read leaves the slot's upper half unspecified).
- Read words out of byte buffers with `memcpy`, as `llg_dpi.c` and
  `value_gmp/consumer_bridge.c` do. A misaligned dereference is undefined on
  every target; it happens to work for ordinary loads on x86_64 and AArch64, but
  AArch64 faults on misaligned atomic and exclusive accesses. The x86_64 UBSan
  job (`-fsanitize=undefined`) reports misaligned dereferences.
- `__int128` exists in GCC and Clang on both ISAs, never in MSVC; the runtime
  does without it (`value/operations.c` divides with `uint64_t` estimates). The
  runtime uses no x86 or NEON intrinsics; the bundled GMP subset is generic C,
  with `longlong.h` inline assembly per ISA under GCC/Clang. Put any new
  compiler-specific code behind `llg_compiler.h`.

### Memory, floating point and pages

- x86_64 keeps loads and stores in program order except a store followed by a
  load; AArch64 may reorder all four combinations. Threads (`llg_dpi_context`
  call contexts, the `llg_wave.c` writer thread) share state only through a mutex or the
  `llg_platform.h` atomics (acquire loads, release stores,
  `llg_atomic_store_load_fence` for a store-then-load handshake), never through
  plain or `volatile` variables: a missing barrier that x86_64 forgives is a
  lost wake-up or a stale read on arm64. MSVC's ARM64 default `/volatile:iso`
  gives `volatile` no ordering; the MSVC branch uses full-barrier `Interlocked`
  intrinsics. Safe Rust is unaffected; review any `Ordering::Relaxed` that
  publishes data.
- Floating-point contraction is off (user decision 2026-10-29): models and the
  runtime build with `-ffp-contract=off` on GCC, Clang and AppleClang, so each
  `real` operator rounds on its own and results are bit-identical on every
  platform and compiler. IEEE 1800 defines `real` as an IEEE 754 double with
  separate operators. Clang otherwise contracts `a * b + c` within one
  expression into a fused multiply-add (`-ffp-contract=on`), which every
  AArch64 has and baseline x86_64 lacks, and GCC contracts in GNU C mode; `real`
  math is rarely a simulation hot spot. The generated project appends the flag
  after `LLG_CFLAGS`, so a user flag cannot re-enable it, and the runtime cache
  key covers it. MSVC needs none from Visual Studio 2022 on: its default
  `/fp:precise` generates no contractions there (Microsoft's `/fp` reference; older
  versions could contract under `/fp:precise`, so build models with 2022 or newer).
  User DPI/VPI C libraries are built by the user and keep the user's flags.
  Never enable contraction or `-ffast-math` for model or runtime sources.
- A NaN made by an invalid operation (`0.0/0.0`, `inf - inf`) has the sign bit
  set on x86_64 (SSE default NaN) and clear on AArch64: glibc prints `-nan`
  against `nan`, and `$realtobits` differs. Fixtures print NaN properties
  (`sim_005/real_numeric_extremes`), never a computed NaN's text or bits.
  Subnormals are kept on both (no flush-to-zero) unless `-ffast-math` is used,
  which model flags never add.
- Page size is 4 KiB on x86_64, 16 KiB on Apple arm64 and 4 KiB or 64 KiB on
  Linux arm64 kernels: never hard-code 4096; `src/ffi/process_memory.rs` asks
  `sysconf(_SC_PAGESIZE)`. Thread stack defaults depend on the C library, not
  the ISA (musl 128 KiB, macOS 512 KiB); threads that run HDL code reserve their
  stack explicitly (`LLG_DPI_CONTEXT_STACK_BYTES`).

## CI shell and Python

- GitHub Actions runs `shell: bash` steps as `bash -eo pipefail`, and `ls` exits
  2 for a missing operand, so `x=$(ls a b 2>/dev/null | head -n 1)` ends the step
  silently; test each candidate with `[ -f ]` in a loop.
- `scripts/` helpers and their unit tests run on the lint job's Ubuntu 22.04
  Python 3.10 and on Rocky Linux 9's Python 3.9: no `tomllib` (3.11, fall back
  to `tomli`), `match` (3.10) or `int.bit_count` (3.10), and no
  `Path.write_text(newline=)` (3.10). Run
  `python3 -m unittest discover -s scripts -p 'test_*.py'` under a 3.9
  interpreter when a script changes.
