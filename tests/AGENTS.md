# Repository validation

Prefer deterministic Rust unit/integration tests, including Rust-side FFI probes.
Checked-in `.v`/`.sv` files go directly to `llg` for end-to-end simulation: never
embed/generate those designs in Rust. Rust owns independent expected results.
Focused frontend/DB/IR tests may use in-memory sources. Serialize parent CWD changes
with a fresh temp directory and unwind-safe restoration. Use `compile_checked` for
successful execution/elaboration; raw `compile` is for partial snapshots/diagnostics,
including proof that checked compilation withholds blocking-error snapshots.

## Simulator and native acceptance

- Shared `support/sim_cli.rs` suites require CMake, isolated child directories,
  both default and `--no-opt` public CLI runs, exact stdout, expected stderr and
  status, and specific frontend/lowering/runtime diagnostics. Positive migration
  or unsupported-feature rejection is failure, never a skip/success substitute.
  Preserve each suite's existing frontend, timeout and sanitizer contracts.
- Independent oracles use explicit truth tables, bit strings and width/sign
  arithmetic, not production implementation helpers. Keep specification-derived
  X/Z, scope, signedness and capacity boundaries. Expected-output changes need
  feature/clause justification; never automatically bless them. Prefer behavioral
  assertions to private generated-name substrings.
- `support_harness` checks CWD/mutex unwind recovery and descendant timeout cleanup:
  POSIX process groups with kill, Windows taskkill /T. Conformance inputs must
  exist and be staged. Before publishing run
  `python3 scripts/check_sim_fixture_integrity.py --tracked` and its Python unit
  tests from [the test guide](readme.md). The checker recognizes static harness
  forms, not arbitrary Rust expressions; extend recognizers for new forms.
- `sim_dynamic_ownership` is positive public-HDL acceptance. Keep original
  runtime/waveform assertions, exact CTest inventories and cleanup tests active;
  smaller components, parser-only checks or recognized source shapes do not
  replace them. `runtime_value_storage/validate.py --full` requires Rust, real
  emitted-model tests, CLI acceptance and repository tests. Missing prerequisites
  are blocked, not passed; reject zero/missing/duplicate/disabled tests and stale
  products. Component sanitizer probes and generated-model sanitizer execution
  are separate gates.
- `slang_frontend`, `slang_semantics` and `model_tests` independently exercise
  safe capture, typed relationships, complete delays, declaration-owned ranges,
  source topology and copy-before-destroy; downstream tests use validated DB/IR
  builders. Unsupported capture is not silently complete. `emit_decoupling`
  source checks and fragment-order checks are architecture checks, not execution.
- `property_elab` mirrors identical Rust value vectors in the C self-test;
  retain independent arithmetic/real conversion/net-resolution probes.
  `runtime_values` compiles values without scheduler/coroutine runtime; random tests cover
  Annex N and process-stream rules, including explicit seeds/ranges at -O0/-O2.
- Preserve `sim_cmake` missing-CMake graceful skips and its source-generation,
  generator, configure-error and stale-source checks. Native DPI shared-library
  cases skip on non-Unix or missing CMake/C compiler; do not generalize these
  exceptions to mandatory public conformance suites.
- The build applies `patches/libfst` in place in the tracked `vendor/libfst`, so
  working trees may show it modified. Tests accept the clean or fully applied
  tree and reject partial ones; `vendor_patches` also fails when HEAD or the
  index holds the applied state, so never commit it (see
  [patch docs](../patches/README.md#libfst)). Clean-tree gates must tolerate or
  restore it deliberately, as `scripts/run-regression.sh` does.
- Preserve supplied imported sources/provenance. C VPI fragments are not standalone
  plugins; handwritten output-shape probes are not Rust-emitted C. Recreated
  missing inputs are replacement witnesses, not recovered historical evidence.

Suite ownership and focused commands are in [readme.md](readme.md); datatype
fixture guides retain their local LRM/oracle contracts. Do not duplicate progress
reports or complete feature inventories here.

## Feature completion change checklist

For each feature, follow the owning source guides and complete the affected steps;
record a reason when a step or failure case does not apply.

- Capture through `Db::from_slang`; copy values, relationships and source locations
  into owned descriptors. Prove consumers survive native snapshot destruction.
- Add typed semantic/execution operations and validate IDs, types, shapes and
  contexts. Extend effect/dependency analysis and every optimizer traversal.
- Emit from owned ExecutionModel data through the whole-model emitter. A legacy
  C fragment or handwritten runtime probe alone is not feature acceptance.
- Define runtime initialization, copy/alias ownership, suspension, cancellation
  and destruction. Feature code uses public `sv4_*` operations and the neutral
  `llg_sv4_*` bridge; follow [the value facade](../src/sim/rt/value/facade.md),
  including independent packed-value owners. Do not inspect private payloads.
- Embed new runtime fragments in `src/sim/rt/mod.rs` and the generated-model CMake
  inputs; runtime C stays out of Rust binaries. Keep fragment-order tests,
  `emit_decoupling` and `generated_c_frame_lint` active.
- Add checked-in positive, nearest-illegal, boundary and composition fixtures to
  the explicitly declared task module described in [the test guide](readme.md).
  Use public CLI acceptance and independent clause-derived oracles; missing tools
  block acceptance. Label parser/DB/IR/handwritten-C tests `component_*` and report
  them separately from generated-model execution.

| Failure or boundary | Required coverage when affected |
| --- | --- |
| Invalid IDs/shapes | Focused owned DB/IR validator tests reject malformed descriptors before emission. |
| Allocation failure | Use the affected allocator's existing fault-injection path; verify partial initialization cleanup and unchanged destinations. If none exists, report the missing probe rather than infer success. |
| Suspension/cancellation | Public HDL cancels at each new suspension point; verify wakeup detachment, no late publication and exactly-once cleanup. |
| Aliasing | Public HDL covers overlapping source/destination, copied values and captured values surviving mutation. |
| Source maps/lifetimes | Generate after snapshot destruction and build/execute after Db destruction with `sim_cli::run_case_after_db_drop`; check surviving fixture identity. Add focused source-location assertions for new descriptors. |
| Optimizers/native builds | Public fixtures run with default and `--no-opt`; representative affected paths also run `--model-opt-level O0` and `O3`. The owned-lifetime helper runs all four combinations. |

Serialize edits to shared Db/IR enums and validators, facade headers and neutral
bridges, emitter facades, scheduler roots, `src/sim/rt/mod.rs`, Cargo/CMake/build
files and suite declarations. Coordinate the affected hunk with its owner and the
integration orchestrator; keep changes additive and small, and use separate
native build trees. Feature work need not wait for the entire GMP track.

## `llg.toml` coverage

One schema (`llg::config`) serves `llg` and `llg_ls`. Library `config::tests` own
schema, path-resolution, unknown-key and error-context cases; `llg` `settings::tests`
own command line > config > environment precedence (scalars replace, repeatable
lists replace, files replace sources, boolean opposites); `llg_config_cli` runs the
public driver against `fixtures/config_cli` for discovery, `--config`, missing/bad
files and per-kind overrides; `dump_tokens`/`lsp_stdio` prove the server accepts
driver-only keys and rejects unknown ones atomically. A new key needs coverage in
each layer it touches.

## LSP acceptance and fixtures

`lsp_stdio.rs` and its domain modules launch `llg_ls` over framed standard JSON-RPC,
never stdout debug text. Gate `lsp_stdio`, `shadowing` and `dump_tokens` by `lsp`,
so no-default builds cannot launch stale binaries. Preserve these wire contracts:

- Independent multi-root scans, default/client-overridden `llg.toml`,
  `llg.configFiles`, per-root lint and hot reload; `llg/configChanged` fires only
  for changed parsed config. Dynamic config/source/include watchers re-register
  after feature-data commits; dependency changes refresh every dependent root.
- `.v`/`.sv` units, include-only headers/arbitrary extensions, include/exclude
  precedence and longest-root ownership transfer on workspace add/remove.
  Unsaved sources/headers are authoritative; reject include escapes, use read-only
  staging and create no compiler artifacts in workspace CWD.
- Shared-file diagnostics deduplicate identical findings and label distinct ones
  `[<root-name>]`. Owner-wins tokens/hover labeling; publish unopened files,
  watched fixes and stale-diagnostic clearing. New lint rules need triggering and
  nearby quiet controls with identical stable IDs in CLI JSON and LSP publication/
  config tests.
- Failed compiles preserve last-good navigation. Non-syntax errors with surviving
  data retain features; syntax-broken roots retain declaration symbols/hover where
  possible, fatal-only roots do not. Watched fixes upgrade analysis. Current
  syntax-invalid open buffers return authoritative empty semantic tokens.
- Exact instance-scope bindings: named port/parameter labels refer to child
  declarations, actuals/override RHS to parent scope. Check single/multiline and
  syntax-fallback forms with dumpTokens `bind=`. Module types stay distinct from
  namesake instances; enum navigation also has wire coverage.
- `lsp_stdio/genvar.rs` covers explicit/inline genvars in hover/definition/
  references/prepareRename/rename/symbols/tokens, including unused/pruned scopes,
  ordinary namesakes, labels/members, fallback and UTF-16 columns. Keep
  `fixtures/lsp/genvar/` and `genvar-fallback/` inputs.
- Explorer roots honor configured tops/source graphs; preserve recursive children,
  leaves, typed contents, declaration fallback and useful roots under truncation;
  never return shadow URIs. Tokens keep parameters/localparams `property.readonly`
  in dimensions/expressions/actuals and data/net/directions `type`, for committed
  and isolated buffers, including missing children. Retain compact-capture tests.

Fixtures use `fixtures/lsp/test.json`, schema `llg.lsp.fixture/v1`, effective
per-root `llg.toml` and `// llg-lsp-fixture:` headers. Synchronize the separate
module-explorer manifest/header convention with its suite. Keep domain facades
explicit; nested implementation files must not become accidental Cargo test targets.

## Safeguards

Align tests with [shared memory policy](../src/AGENTS.md),
[LSP admission](../src/bin/llg_ls/lsp/AGENTS.md) and
[request/cache/explorer policy](../src/bin/llg_ls/AGENTS.md), plus their source
implementations. Cover exact input/response budgets, cache backpressure, logging
and startup integration:

```sh
cargo nextest run --locked --lib memory_limit::tests
cargo nextest run --locked --lib ffi::process_memory
cargo nextest run --locked --test sim_memory_guard
cargo nextest run --locked --bin llg_ls input_budget
cargo nextest run --locked --bin llg_ls oversized
cargo nextest run --locked --bin llg_ls response_budget
```

## CI and release gate

Daily Cargo/nextest/runner tests use the optimized `test` profile, matching the
release optimization level while retaining debug assertions and overflow checks.
Use `cargo test --profile quick`, `cargo nextest run --cargo-profile quick` or
`scripts/run-tests.sh --cargo-profile quick` for shorter edit-test rebuilds.
`quick` leaves workspace code unoptimized; Cargo caches optimized dependencies.
`quick` adds `target/quick/` beside the default `target/debug/`; budget disk for
both. Nextest's `--profile` selects runner settings rather than Cargo profiles.
Use the optimized default for the repository gate; see [build profiles](readme.md#cargo-build-profiles).

For concurrent worktrees using tmpfs, run `scripts/run-tests.sh --test-work-dir PATH`
from each worktree; append normal nextest selection/concurrency arguments. The
path is explicit and must exist and permit execution. Use the same root to share
compatible runtime-cache entries; the runner isolates worktree/run scratch and
keeps Cargo target/build directories in each worktree. Do not override those
directories into a common tree. Keep logs and evidence in local `persistence/`.
Successful runs remove their scratch; failures/interruption may leave it for
inspection. Remove only inactive runs owned by your task; prune the shared
runtime cache only when all users have stopped. Four runners at eight threads
mean 32 concurrent tests, with no global quota or automatic cache eviction.
See [the storage guide](readme.md#parallel-worktrees) for commands and limits.
Optional Rust/native caches and the Linux GNU mold linker are documented in
[development accelerators](readme.md#optional-development-accelerators).
Runner flags `--sccache`/`--mold` share the plain-Cargo environment helper;
leave them disabled unless requested and fail for missing requested tools.

Run the complete [repository gate](readme.md#repository-gate), not fmt/check/clippy
alone. [ci.yml](../.github/workflows/ci.yml) defines Ubuntu gates and five-platform
builds on master pushes, manual dispatch and published Releases (including
prereleases), not draft saves or standalone tag pushes. Every platform splits
building from testing: `build` jobs compile the executables, run the platform
audits and package, and `cargo nextest archive` the release-profile test
binaries; parallel `test` jobs (Windows/macOS) and `linux-test` jobs (Linux)
download that archive and run the full suite with `cargo nextest run
--archive-file` (`--no-fail-fast`) before release. Linux builds only in the
static-musl Alpine container (`linux-build`); its `linux-test` jobs run the
archive on Ubuntu 24.04 and Rocky Linux 9 containers per architecture. Windows
and macOS test jobs run on the build job's runner kind with the checkout
extracted over the same path, so compile-time paths (`CARGO_MANIFEST_DIR`,
`CARGO_BIN_EXE_*`) resolve; they install nextest, Python and (Windows) the MSVC
tools, apply the libfst patch to the fresh checkout, and need no Rust toolchain.
Archives are retained one day. CI selects the `ci` nextest profile (one test per
logical CPU). Observed platform
differences and their portable fixes are in
[shared source policies](../src/AGENTS.md#platform-differences).

Lint/sanitizer jobs disable Rust debug info/incremental compilation and strip native
frontend debug sections while retaining debug assertions/overflow checks. Preserve
nextest's bounded heavyweight group, compatible shared runtime caches and failure-
time disk/memory telemetry. Generated C sanitizer flags remain separate.

The 180-minute generated-runtime-sanitizers job runs runtime_values, runtime_random,
runtime_boundaries, sim_counter, sim_data_types, sim_data_types_next,
sim_data_types_completion, sim_type_conformance, sim_partial_features,
sim_net_resolution, sim_net_defaults, sim_function, sim_loops and
sim_procedural_assign with GCC ASan/UBSan; this is not LSP admission coverage.
The 15-minute dependency-audit runs cargo audit on those triggers and Mondays
04:17 UTC. Reports stay in workflow logs, not uploaded artifacts.

Caching (binding user rule): CI caches compiled third-party Cargo dependencies and
the C of generated test models, never anything built from this repository. No
sccache/ccache wraps llg or the native Slang/fmt/wrapper build (leave `LLG_CCACHE`
and `CMAKE_C_COMPILER_LAUNCHER` unset in CI), and no llg, `target/slang`, test
executable, nextest archive or runtime cache is saved.
- Rust jobs (lint, sanitizers, `build` matrix, manual full-host) use
  `Swatinem/rust-cache@v2` with `cache-targets: "true"`, `prefix-key: deps-v1`, a
  distinct `shared-key` per job/target/profile and `save-if` master. The musl
  `linux-build` has no such action: it restores/saves `target` and the mounted Cargo
  registry with `actions/cache/{restore,save}` keyed on target, image and
  `Cargo.lock`/toolchain/config hashes. Every Rust job ends with
  `scripts/ci_prune_cargo_cache.py` (`sudo` for the root-owned container tree)
  before the save: it keeps only artifacts of Cargo.lock packages that have a
  registry/git source, so the workspace crate, its build-script output,
  `target/slang`, test executables and archives never persist (the rust-cache pass
  alone would leave an empty Slang tree and does not cover the musl job).
  Compiled dependencies are about 0.3 GB per entry.
- Generated models: `.github/actions/ccache-setup` downloads a pinned,
  checksum-verified ccache (`scripts/ci_ccache.py`), restores its directory with
  `actions/cache` (key per job/target/distro and ISO week, 500 MB limit), proves a
  cross-directory hit with the compiler and generator the tests use, then exports
  `LLG_C_LAUNCHER` and `CCACHE_*` (`CCACHE_BASEDIR` = temp dir, `CCACHE_NOHASHDIR`).
  A failed install or self-check leaves the launcher unset (uncached run, warning).
  Used by lint, sanitizers (flags are in the hash), Windows/macOS `test` and
  `linux-test` (installed on the runner, activated inside the container).
  `ccache-finish` prints `ccache -sv` and, on master only, saves when the week's
  key is new. Hit rates are in each job's statistics step. `release`, `build`,
  `linux-build` and dependency-audit run no generated models.
Only release events upload Actions
packages, retained one day; pushes/manual builds upload none. Retention does not
cap the documented account-wide 500 MB artifact allowance across concurrent runs/
repositories. Runner disk is a separate resource. The repository's 10 GB cache
budget holds about nine Cargo entries (about 0.3 GB) plus nine ccache entries (at
most 500 MB each, usually less); Python tests for the scripts run in `lint`
(`scripts/test_ci_cache.py`).

Matrix: Linux x86_64/arm64, Windows x86_64/arm64, macOS arm64. Audit architecture,
static Linux/static Windows CRT linkage, system-only Windows/macOS imports and
startup before packaging. Alpine also lists cli_info from a small release test
binary to catch allocator/libc startup failures. The release job waits for every
test/audit/build, checks all five checksums and attaches to the existing Release.
Only it receives contents:write; never create/publish/edit release metadata.
Reruns replace matching asset names. Package naming is
`lapligence-<version>-<os>-<arch>.<ext>`: strip leading v, use linux/windows/macos,
x64/arm64 and tar.gz on Unix/zip on Windows; include both executables, readme.md,
LICENSE and checksum files. Configuration is not validated platform support;
keep evidence in ignored `persistence/platforms.md`.

## Documentation ownership

Keep concise test methodology/commands/limitations in `tests/readme.md`, simulation
feature status only in `docs/sim_features.md`, and dated plans/findings/run evidence
in ignored `persistence/`. Preserve module and fixture safety rules when compacting
human documentation.
