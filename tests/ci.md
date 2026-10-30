# CI and release

[ci.yml](../.github/workflows/ci.yml) and the reusable
[native-platform.yml](../.github/workflows/native-platform.yml) (Windows/macOS)
and [linux-platform.yml](../.github/workflows/linux-platform.yml) (Linux) own
job selection; their header and step comments explain each job. This page lists
the rules a change must keep. Platform pitfalls are in
[cross-platform handling](../docs/cross_platform.md).

## Triggers and pipelines

- Ubuntu gates and five-platform builds run on master pushes, manual dispatch
  and published Releases (including prereleases), not draft saves or standalone
  tag pushes.
- Each platform (`windows-x86_64`, `windows-arm64`, `macos-arm64`,
  `linux-x86_64`, `linux-arm64`) is an independent caller job. Its `build` job
  compiles the executables, runs the platform audits, packages, and archives the
  release-profile test binaries (`cargo nextest archive`); its `test` job needs
  only that `build`, downloads the archive and runs the full suite
  (`--no-fail-fast`). `release` needs all five callers plus lint, sanitizers and
  audit.
- Reusable workflows do not inherit the caller's top-level `env`; each sets
  `NEXTEST_PROFILE`. Artifact names are unique per platform
  (`nextest-archive-<target>`, `lapligence-<os>-<arch>`); archives are kept one day.
- CI selects the `ci` nextest profile (one test per logical CPU). Tests that
  build C with CMake also use every CPU each, so on a 4-CPU runner one compiler
  process can get a small CPU share: a per-command timeout needs headroom over
  the locally measured cost (run 110830171169: a 6.6 s Clang `-O2` compile of
  `llg_rt.c` exceeded 60 s beside three CMake probe builds; the test now compiles
  at `-O0`).
- Linux builds only in the static-musl Alpine container; its `test` jobs run the
  archive on Ubuntu 24.04 and Rocky Linux 9. Windows/macOS `test` jobs run on the
  build job's runner kind with the checkout at the same path, so compile-time
  paths (`CARGO_MANIFEST_DIR`, `CARGO_BIN_EXE_*`) resolve; they need no Rust
  toolchain but apply the libfst patch to the fresh checkout.
- A caller may split its test job with `test_shards`; no platform does now.
- Windows-only diagnostics (`scripts/ci_windows_runner.py measure`,
  `scripts/ci_resource_sampler.py`, `report` steps) are read-only and never fail
  a job.

## Windows on ARM

`llg_ls` only; `llg` unsupported.

- The `windows-arm64` caller passes `lsp_only: true`: `build` runs only
  `cargo build --bin llg_ls`, skips the C11 runtime gate, model builds and cost
  probe, audits and smoke-tests `llg_ls.exe`, and packages `llg_ls.exe`,
  `readme.md` and `LICENSE`.
- Its archive covers `--lib --bin llg_ls --test general` (Cargo still compiles
  `llg` for `CARGO_BIN_EXE_llg`; it is never run, audited or packaged). One
  `test` job runs nextest profile `ci-lsp`, whose `default-filter` in
  `.config/nextest.toml` is the language-server test set.
- A new LSP/frontend-only `general` suite joins that filter; `test_layout` rejects
  stale names and simulator-using suites. Never add simulator-only tests to it.
  Local run: [prerequisites](readme.md#prerequisites).

## Value backends and other jobs

- Every test lane runs models with compact values and GMP kernels (the only user
  configuration), compiling the bundled `vendor/gmp` subset; no GMP is installed.
  Legacy and portable backends are development references for the parity lanes
  (`LLG_DEV_VALUE_BACKEND`/`LLG_DEV_COMPACT_KERNELS`).
- The lint job runs `scripts/gmp_tables.py --check` and the `scripts/test_*.py`
  unit tests it lists on Ubuntu 22.04's Python 3.10; scripts stay importable
  there and on Rocky Linux 9's 3.9
  ([Python](../docs/cross_platform.md#ci-shell-and-python)).
- A Windows-only failure class (MSVC `/W4 /WX`, CRLF) gets a Linux guard in the
  default suite where one is practical
  ([guards](readme.md#msvc-and-windows-guards-on-linux)).
- Lint/sanitizer jobs disable Rust debug info and incremental compilation and
  strip native frontend debug sections, keeping debug assertions and overflow
  checks. Keep nextest's bounded heavyweight group, compatible shared runtime
  caches and failure-time disk/memory telemetry. Generated-C sanitizer flags stay
  separate.
- The generated-runtime-sanitizers job (180 min) runs the suites listed in
  `ci.yml` with GCC ASan/UBSan; it is not LSP admission coverage. The
  dependency audit (15 min) runs `cargo audit` on the same triggers and Mondays
  04:17 UTC; reports stay in workflow logs.
- `dependency-audit.yml` (Mondays 05:23 UTC and manual) is the upgrade reminder:
  `scripts/ci_dependency_report.py` reports RustSec vulnerabilities and
  warnings, outdated direct crates, the toolchain pin, submodules behind their
  newest upstream tag and outdated action majors without building the crate.
  Vulnerabilities and failed checks fail it; upgrades only remind. Only its
  `issue` job gets `issues:write`, and only on master it opens, updates or
  closes the one `dependency-audit` issue.

## Caching (binding user rule)

- Cache only compiled third-party Cargo dependencies, the third-party GMP prefix
  of the GMP lanes and the C of generated test models; never anything built from
  this repository.
- No sccache/ccache wraps llg or the native Slang/fmt/wrapper build: leave
  `LLG_CCACHE` and `CMAKE_C_COMPILER_LAUNCHER` unset. No llg, `target/slang`, test
  executable, nextest archive or runtime cache is saved.
- Every Rust job ends with `scripts/ci_prune_cargo_cache.py` before the save. The
  musl `build` job uses `actions/cache/{restore,save}` instead of
  `Swatinem/rust-cache`.
- Generated-model ccache: `.github/actions/ccache-setup` and `ccache-finish`
  (pinned, checksum-verified, self-checked; a failed self-check leaves the
  launcher unset). Only master saves entries.
- The repository's 10 GB cache budget holds about nine Cargo entries (~0.3 GB)
  plus the ccache entries; GitHub evicts superseded entries first.

## Release packages

- Audit architecture, static Linux/static Windows CRT linkage, system-only
  Windows/macOS imports and startup before packaging. Alpine also lists
  `cli_info` from a small release test binary to catch allocator/libc startup
  failures.
- `release` waits for every test/audit/build, checks all five checksums and each
  archive's exact file list, and attaches to the existing Release. Only it gets
  `contents:write`; never create, publish or edit release metadata. Reruns
  replace matching asset names. Only release events upload packages (kept one
  day); the account-wide 500 MB artifact allowance is shared.
- Naming: `lapligence-<version>-<os>-<arch>.<ext>`; strip a leading `v`; os
  `linux`/`windows`/`macos`, arch `x86_64`/`arm64`; `tar.gz` on Unix, `zip` on
  Windows. Contents: both executables (Windows arm64: `llg_ls.exe` only),
  `readme.md`, `LICENSE` and checksums.
- Configuration is not validated platform support; keep platform evidence in
  ignored `persistence/platforms.md`.
