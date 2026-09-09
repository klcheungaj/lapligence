# Repository validation

Prefer Rust unit and integration tests, including Rust-side FFI probes.
Tests must be deterministic. Prefer admitted in-memory Slang sources; tests
which change the process CWD must serialize that change, use a fresh temporary
directory, and restore it through an unwind-safe guard. Use `compile_checked`
for successful execution/elaboration. Raw `compile` is for tests inspecting
partial snapshots and diagnostics; assert that the checked contract withholds
a snapshot containing blocking errors.

## LSP acceptance and fixtures

`lsp_stdio.rs` launches **`llg_ls`** and speaks only framed standard LSP JSON-RPC;
never parse/depend on stdout debug output. `lsp_stdio.rs`, `shadowing.rs`, and
`dump_tokens.rs` are gated by the `lsp` feature so no-default builds cannot
launch a stale language-server binary. Cover these contracts:

- Default and client-overridden per-root `llg.toml`, `llg.configFiles`, hot
  reload without restart, `llg/configChanged` only on changed parsed configs,
  per-root lint policies and independent multi-root scans.
- `.v`/`.sv` compilation units, include-only headers/arbitrary extensions,
  include/exclude precedence, longest-root ownership transfer on workspace
  add/remove, and dynamic config/source/include watchers re-registered after
  feature-data-bearing commits. Dependency events refresh every dependent root.
- Shared-file diagnostic unions: identical findings once, distinct findings
  labeled `[<root-name>]`; owner-wins semantic tokens and hover labeling.
  Publish never-opened files, refresh on watched disk fixes and clear stale
  findings. Shared careless-mistake rules have project-wide diagnostic coverage
  and the same stable IDs exercised through simulator `--lint-json`.
- Unsaved source/header buffers, configured-directory include authorization
  and rejected escapes, last-good navigation after failed compiles, read-only
  staging and no compiler artifacts in the server workspace CWD.
- Feature serving for non-syntax error projects with surviving semantic data, and
  declaration-level document/workspace symbols/hover for syntax-broken roots
  (e.g. an unterminated sibling module). Fatal-only roots are feature-less;
  watched fixes upgrade analyses. Syntax-invalid current open buffers yield
  authoritative empty semantic tokens.
- Binding-precise instance-scope navigation; named port/parameter LABELS bind
  to child declarations while ACTUALS/override RHS stay in parent scope.
  Cover single/multiline forms and syntax-fallback bindings with `dumpTokens`
  `bind=` as oracle. Module-type navigation stays distinct from same-named
  instance identifiers. Parse-backed enum navigation also has wire coverage.
- `lsp_stdio/genvar.rs` exercises explicit and inline genvars through standard
  hover/definition/references/prepareRename/rename/symbol/token requests. Keep
  exact scope-isolation assertions, including unused and pruned declarations,
  ordinary namesakes, labels/members, syntax fallback, and UTF-16 columns.
  HDL lives in `fixtures/lsp/genvar/` and `fixtures/lsp/genvar-fallback/`.
- Module explorer: configured-top/source-graph roots, recursive children and
  leaves, declaration fallback, typed contents, no shadow URIs, and useful
  hierarchy roots surviving module-content budget truncation.
- Semantic colors: parameters/localparams remain `property.readonly` in
  dimensions, expressions, and instance actuals; data/net/direction words are
  `type`. Cover cached project and isolated unsaved-buffer responses, including
  absent child modules. Unit tests also cover compact capture and shadowing.

Fixtures use `fixtures/lsp/test.json`, schema `llg.lsp.fixture/v1`, an
effective `llg.toml` per root, and `// llg-lsp-fixture:` source headers. Keep
the dedicated `fixtures/lsp/module-explorer/` manifest/header convention in
sync with its suite. Shared lint additions need one fixture with triggering
and nearby quiet controls, simulator CLI JSON, and LSP publication/config
coverage for the same rule IDs.

## Suite map

- `cli_info.rs` checks help/version output, early exit with stdin held open,
  and usage errors before memory guards, logging or LSP serving start.
- `slang_frontend.rs` probes the release-pinned native
  bridge through safe Rust APIs: owned hierarchy/parameters/constants,
  compiler/analysis diagnostics, checked failure, repeated/concurrent compilation
  isolation, and admitted-buffer includes versus rejected external files.
- `slang_semantics.rs` pins the safe semantic snapshot contract independently
  of database and simulator lowering: process and assignment relationships,
  timing and event edges, type ranges and aggregate members, and paired module
  port declarations and actuals.
- `support_harness.rs` verifies that simulator test CWD restoration and mutex
  recovery remain sound when a test action unwinds.
- `sim_data_types.rs`, `sim_data_types_extended.rs`, and
  `sim_data_type_edges.rs` cover datatype semantics and boundaries; detailed
  contracts are in [data_types/AGENTS.md](fixtures/sim/data_types/AGENTS.md),
  [data_types_extended/AGENTS.md](fixtures/sim/data_types_extended/AGENTS.md),
  and [data_type_edges/AGENTS.md](fixtures/sim/data_type_edges/AGENTS.md).
- `sim_data_types_next.rs` is a bounded next-phase inventory for aggregate and
  container features, with explicit unsupported cases; its contract is in
  [data_types_next/AGENTS.md](fixtures/sim/data_types_next/AGENTS.md).
- `sim_data_types_completion.rs` freezes eight positive completion contracts
  plus one explicit unsupported reduction case; its contract is in
  [data_types_completion/AGENTS.md](fixtures/sim/data_types_completion/AGENTS.md).
- `elab_resolve.rs` exercises resolved Slang parameter values; `config_effect.rs` observes
  configured defines and top-level parameter overrides driving
  generate branches through the owned `DesignModel`.
- `elaboration/run_elab_check.sh` runs `elab_check` over representative designs;
  the binary validates the owned semantic database and prints hierarchy,
  binding and resolved-parameter summaries.
- `sim_counter.rs` is the simulator regression suite: compiles + runs
  real designs end-to-end (codegen → `cc` → execute) and asserts exact stdout
  (hand-simulated traces, documented in the test), plus the C runtime
  self-test (`llg_rt_selftest.c`, sv4 vectors + scheduler checks).
- `region_conformance.rs` pins the IEEE 1800 §4 scheduling-region
  semantics (active/inactive `#0`/NBA ordering, multi-delta settle, fork/join
  timing); a `// REGION-BUG:` case means the scheduler deviates.
- `property_elab.rs` runs proptest properties over `core::elab::Value`
  (X-propagation, resize/concat round-trips, casez/casex truth tables) and
  hosts the generator for the deterministic C vector table checked by
  `llg_rt_selftest.c` — keep elab.rs and the runtime semantically in sync.
- `sim_*.rs` are the per-feature simulator suites (counter, function,
  fork, memory, interface, interface_body, casez, monitor, timescale, stress,
  geninit, varinit, wait, force, hier, inout): each compiles a design,
  codegens, builds the C model through `sim::build::build_model_cmake`, runs
  it and asserts the exact stdout.  Model-building suites require cmake and
  skip gracefully (`SKIP: cmake not available`) when
  `sim::build::cmake_available()` is false.
- `emit_decoupling.rs` pins the pipeline shape with architectural greps:
  `sim::emit_c` consumes only the execution IR, while `sim::codegen` lowers
  the semantic model without emitting runtime C calls directly.
- `sim_opt_differential.rs` runs designs twice — once with
  `OptConfig::default()` (all passes) and once with `OptConfig::none()` —
  building both models via `sim::build::build_model_cmake` and asserting
  byte-identical stdout.
- `sim_variable_lifetime.rs` runs an in-memory Slang design with optimization
  enabled and disabled, proving that resolved static procedural locals retain
  storage across block reentry while automatic locals are recreated.
- `sim_cmake.rs` covers the build path (5 cases: library-level
  end-to-end CMake build, explicit `CmakeBuildOpts` generator backend,
  invalid-generator configure error, driver default, missing-cmake
  actionable error); skips gracefully when cmake is absent.
  Its source-generation check also pins the separate value-runtime translation
  unit and retention of both value files during stale-source cleanup.
- `runtime_values.rs` compiles `llg_value.c` independently of the scheduler and
  libaco, checking packed value operations, real/shortreal conversions, and
  wire/wired-AND/wired-OR truth tables and wide-vector normalization.
  `sim_net_resolution.rs` covers per-site wired drivers, aliases, repeated
  updates, optimizer parity, driver limits and unsupported-context rejection.
  `sim_net_defaults.rs` covers implicit pull/supply ordering, initial defaults,
  driver release and unchanged resolved-value notifications.
- `model_tests.rs` covers the explorer-facing model projection: formal
  ports are not duplicated as backing signals, concrete net kinds are kept,
  and packed ranges remain owned per elaborated instance without absorbing
  unpacked dimensions.
- `sim_memory_guard.rs` exercises the shared `memory_limit` safeguard
  end-to-end via `LLG_MEMORY_LIMIT_MB`.
- `sim_waveform.rs` covers HDL→VCD/FST dump controls, X/Z and real values,
  hierarchy, timestamps and final blocks. The waveform runtime self-test owns
  ring wrap/backpressure, flush acknowledgement, aliases and FST reader reopening.
- `sim_packed_strings.rs`, `sim_bit_queries.rs`, `sim_real_conversions.rs`,
  `sim_wildcard_eq.rs`, and `sim_loops.rs` compare optimized/unoptimized
  execution for packed strings, bit queries, numeric conversions, wildcard
  equality/case-inside, and lexical loop declarations/fixed-array foreach.
  `sim_delay.rs` covers typed constant delay expressions and explicit dynamic,
  negative and unsupported-control boundaries.
  `sim_time_literals.rs` checks typed unit-suffixed, scientific and real
  parameter delays, lexical shadowing, and rounding of completed delays to the
  local precision before global scheduling, with optimizer parity.
  `sim_time_values.rs` covers Slang's typed time-literal values, module-unit
  scaling without local precision rounding, integral assignment conversion,
  and ownership after admitted source buffers are removed.
  `sim_fill_literals.rs` checks context-determined fills through expressions
  and case operands, self-determined boundaries, and wide-operation rejection.

## Safeguard validation

Contracts live in [../src/AGENTS.md](../src/AGENTS.md) (shared process memory
and review checklist), [LSP backend](../src/bin/llg_ls/lsp/AGENTS.md)
(input admission/staging/config), and [LSP guide](../src/bin/llg_ls/AGENTS.md)
(request limits, cache backpressure, explorer serialization and logging).
Keep these aligned with `memory_limit.rs`, `ffi/process_memory.rs`,
`llg_ls/config.rs`, `lsp/handlers.rs` and its children, and `module_explorer.rs`.
The driver startup integration probe is `sim_memory_guard.rs`.

```sh
cargo test --lib memory_limit::tests -- --test-threads=1
cargo test --lib ffi::process_memory -- --test-threads=1
cargo test --test sim_memory_guard -- --test-threads=1
cargo test --bin llg_ls input_budget -- --test-threads=1
cargo test --bin llg_ls oversized -- --test-threads=1
cargo test --bin llg_ls response_budget -- --test-threads=1
```

## CI and release gate

[ci.yml](../.github/workflows/ci.yml) runs the Ubuntu test gates and five-platform
build matrix on pushes to `master`, manual dispatch for the selected branch,
and GitHub Release publication (`release: published`, including prereleases).
Draft saves and standalone tag pushes do not trigger CI. Before release, run
its complete serialized `lint` gate:

```sh
cargo fmt --check
cargo check --all-targets --all-features
cargo check --lib --no-default-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features -- --test-threads=1
```

The Ubuntu lint and sanitizer jobs disable Rust debug info and incremental
compilation and strip debug sections (including the linked native frontend's)
from Rust executables. They limit Cargo test builds to two concurrent jobs,
independently of the serialized test execution above, and report disk/memory
use even after failures. Debug assertions and overflow checks remain enabled;
generated C sanitizer flags are unchanged.

Workflow caches retain Cargo downloads only, excluding compiled targets and
installed Cargo binaries to reduce use of the repository's 10 GB cache budget.
Only release events upload packages as Actions artifacts, which expire
after one day; branch pushes and manual builds upload no artifacts. Retention
does not enforce the account's 500 MB artifact budget across concurrent runs or
other repositories. Runner working-disk usage is separate from these quotas.

The `generated-runtime-sanitizers` job has a 180-minute limit and
runs `runtime_values`, `runtime_boundaries`, `sim_counter`, `sim_data_types`,
`sim_data_types_next`, `sim_data_types_completion`, `sim_function`, and
`sim_loops` with GCC ASan/UBSan.
This checks
generated C/runtime memory safety, not LSP admission. The 15-minute
`dependency-audit` job runs `cargo audit` on those triggers and Mondays at
04:17 UTC. Neither uploads reports; workflow logs are evidence.

The `build` matrix in [ci.yml](../.github/workflows/ci.yml) is configured to
produce release binaries for Linux x86_64/arm64,
Windows x86_64/arm64, and macOS arm64. It checks target architecture, fully
static Linux linkage, static Windows CRT linkage, system-only Windows/macOS
dynamic imports, and a driver startup smoke test before packaging both
executables with checksums.
On release publication, `release` waits for every test/audit/build job, checks
the five package checksums, and attaches packages and checksum files to the
existing GitHub Release. Only that job receives `contents: write`. CI never
creates or publishes a release or edits its metadata; reruns replace assets
with matching names.
Packages use `lapligence-<version>-<os>-<arch>.<ext>`, removing the tag's leading
`v`, with `linux`/`windows`/`macos`, `x64`/`arm64`, and `tar.gz` for Unix or
`zip` for Windows. Each contains both executables, `readme.md`, and `LICENSE`.
Keep platform claims aligned with local `persistence/platforms.md` evidence.
