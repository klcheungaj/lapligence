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
  products. Non-fiber sanitizer components and native stack-switch tests are
  separate gates.
- `slang_frontend`, `slang_semantics` and `model_tests` independently exercise
  safe capture, typed relationships, complete delays, declaration-owned ranges,
  source topology and copy-before-destroy; downstream tests use validated DB/IR
  builders. Unsupported capture is not silently complete. `emit_decoupling`
  source checks and fragment-order checks are architecture checks, not execution.
- `property_elab` mirrors identical Rust value vectors in the C self-test;
  retain independent arithmetic/real conversion/net-resolution probes.
  `runtime_values` compiles values without scheduler/libaco; random tests cover
  Annex N and process-stream rules, including explicit seeds/ranges at -O0/-O2.
- Preserve `sim_cmake` missing-CMake graceful skips and its source-generation,
  generator, configure-error and stale-source checks. Native DPI shared-library
  cases skip on non-Unix or missing CMake/C compiler; do not generalize these
  exceptions to mandatory public conformance suites.
- Preserve supplied imported sources/provenance. C VPI fragments are not standalone
  plugins; handwritten output-shape probes are not Rust-emitted C. Recreated
  missing inputs are replacement witnesses, not recovered historical evidence.

Suite ownership and focused commands are in [readme.md](readme.md); datatype
fixture guides retain their local LRM/oracle contracts. Do not duplicate progress
reports or complete feature inventories here.

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

Run the complete [repository gate](readme.md#repository-gate), not fmt/check/clippy
alone. [ci.yml](../.github/workflows/ci.yml) defines Ubuntu gates and five-platform
builds on master pushes, manual dispatch and published Releases (including
prereleases), not draft saves or standalone tag pushes.

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

Cache Cargo downloads only, excluding compiled targets/installed binaries, within
the repository's documented 10 GB cache budget. Only release events upload Actions
packages, retained one day; pushes/manual builds upload none. Retention does not
cap the documented account-wide 500 MB artifact allowance across concurrent runs/
repositories. Runner disk is a separate resource.

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
