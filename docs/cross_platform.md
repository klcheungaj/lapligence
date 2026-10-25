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
- Compare captured child output with LF oracles only after
  `ffi::platform::native_text_to_lf`.
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
- Text-mode consoles and files write CRLF. `$fopen` passes its mode to C `fopen`
  unchanged, so text-mode `$ftell`/`$fseek`/`$rewind` positions differ; fixtures
  and `lrm_decisions` cases that depend on byte positions or unformatted data
  open files with `"rb"`/`"wb"` (IEEE 1800-2009 21.3.1).

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
  `static const T x;` (C4132; define the object first). Clang
  `-Wconditional-uninitialized`/`-Wunreachable-code` approximate C4701/C4702.
  A cast that truncates a complemented constant, `(uint8_t)~FLAG`, is C4310 (GCC
  and Clang never warn on an explicit cast); mask first, `(uint8_t)(x & ~FLAG)`
  (`runtime_strict_sources::runtime_sources_have_no_truncating_constant_casts`).
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
- `cl /LD` implies the static CRT, whose `stdout` buffer the model cannot flush;
  test DLLs build with `/MD` (`tests/support/c_compiler.rs`), and
  [known issues](known_issues.md) tells users the same.

## macOS

- `/var`, `/tmp` and `/etc` are symlinks into `/private`, and `temp_dir()` is
  `/var/folders/...`; handle-derived paths are resolved (see
  [paths](#paths-and-text)). Linux hits the same mismatch for symlinked project
  directories; reproduce by pointing `TMPDIR` at a symlinked directory.
- `/dev/fd/N` entries are fdesc device nodes, not symlinks; `secure_fs.rs`
  documents the `F_GETPATH`/`fdopendir` handling. Linux `/proc/self/fd/N` is a
  real link whose reopen creates a new description.
- The Annex I sample `svdpi.h` falls back to `<sys/types.h>`, which lacks the
  fixed-width types on macOS; llg writes its own `rt/svdpi.h` with
  `<inttypes.h>`. `-U__linux` on Linux reproduces the failure.
- Apple ld64 does not check a dylib's undefined symbols; llg reads Mach-O symbol
  tables before the build (`src/sim/build/dpi_link.rs` module docs).
  `clang --target=arm64-apple-macos11 -c` makes Mach-O objects on Linux for
  testing that reader.

## Linux musl and glibc

- Linux CI builds only in static-musl Alpine; glibc Ubuntu/Rocky containers run
  those binaries. Minimal images lack clang, which `generated_c_frame_lint`
  needs beside gcc; install test tools explicitly.
- zlib is bundled, never a system package. Without its configure step
  `Z_HAVE_UNISTD_H` is unset and GCC 14/Clang reject its `gz*` code; the bundled
  build defines it outside Windows.
- The build container runs as root and Cargo extracts crates with their archive
  modes (some owner-only), so the workflow hands the registry back to the runner
  user before the cache save.
- On musl, root `build.rs` and `mimalloc_shim.c` wrap C `malloc`/`free` for every
  binary ([executables](../src/bin/readme.md)).

## arm64

- Windows on ARM (`aarch64-pc-windows-msvc`): `llg_ls` only. `llg`, generated
  models, DPI/VPI and the C runtime are unsupported;
  `ffi::platform::simulator_unsupported_reason` makes `llg` exit 2. The library,
  frontend, Db and lint code used by `llg_ls` must stay portable there. CI lane:
  [Windows on ARM](../tests/ci.md#windows-on-arm).
- Plain C `char` is unsigned on AArch64 Linux (also PowerPC, RISC-V, MSVC `/J`)
  and signed on x86, Apple arm64 and Windows. SV `byte` is C `char` (Table H.1):
  DPI thunks keep `char` prototypes and sign-extend each foreign byte, and C code
  (companions included) reads a `byte` through an explicit sign extension, never
  `%d` of a plain `char`. `-funsigned-char` reproduces AArch64 on x86_64.
- tower-lsp 0.20 runs notifications concurrently, and slower runners (Rocky
  arm64) interleave handlers; state written after an `await` must not let an
  older result replace a newer one (`DiscoveryOrder` in
  `src/bin/llg_ls/lsp/handlers/state.rs`).
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

## CI shell

- GitHub Actions runs `shell: bash` steps as `bash -eo pipefail`, and `ls` exits
  2 for a missing operand, so `x=$(ls a b 2>/dev/null | head -n 1)` ends the step
  silently; test each candidate with `[ -f ]` in a loop.
