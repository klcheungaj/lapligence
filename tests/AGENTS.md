# Repository validation

Prefer Rust unit and integration tests, including Rust-side FFI probes.
Tests must be deterministic. Surelog writes `slpp_all/` into its CWD and has
process-global C++ state: use `std::env::set_current_dir` to a fresh temporary
directory, clean up afterward, and serialize execution (`--test-threads=1`).
Use `compile_checked` for successful execution/elaboration. Raw `compile` is
for tests inspecting partial results/diagnostics; assert that the checked
contract withholds a failed session.

## LSP acceptance and fixtures

`lsp_stdio.rs` launches **`llg_ls`** and speaks only framed standard LSP JSON-RPC;
never parse/depend on stdout debug output. Cover these contracts:

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
  staging and no Surelog artifacts (`slpp_all/`, logs) in server workspace CWD.
- Feature serving for non-syntax error projects with surviving UHDM, and
  declaration-level document/workspace symbols/hover for syntax-broken roots
  (e.g. an unterminated sibling module). Fatal-only roots are feature-less;
  watched fixes upgrade analyses. Syntax-invalid current open buffers yield
  authoritative empty semantic tokens.
- Binding-precise instance-scope navigation; named port/parameter LABELS bind
  to child declarations while ACTUALS/override RHS stay in parent scope.
  Cover single/multiline forms and syntax-fallback bindings with `dumpTokens`
  `bind=` as oracle. Module-type navigation stays distinct from same-named
  instance identifiers. Parse-backed enum navigation also has wire coverage.
- Module explorer: configured-top/source-graph roots, recursive children and
  leaves, declaration fallback, typed contents, no shadow URIs, and useful
  hierarchy roots surviving module-content budget truncation.

Fixtures use `fixtures/lsp/test.json`, schema `llg.lsp.fixture/v1`, an
effective `llg.toml` per root, and `// llg-lsp-fixture:` source headers. Keep
the dedicated `fixtures/lsp/module-explorer/` manifest/header convention in
sync with its suite. Shared lint additions need one fixture with triggering
and nearby quiet controls, simulator CLI JSON, and LSP publication/config
coverage for the same rule IDs.

## Suite map

- `sim_data_types.rs` compiles checked-in `.v`/`.sv` fixtures under
  `fixtures/sim/data_types/` and compares optimized/unoptimized execution.
  Independent full-bit truth-table output and positional arithmetic/cast
  oracles cover model-sized and partial-limb vectors. The original 40-case
  campaign is active without ignored tests and has regular and sanitizer
  coverage. Commands and
  stable coverage notes live in the [fixture guide](fixtures/sim/data_types/readme.md),
  with normative data/width rules in [the semantics reference](../docs/sim_data_semantics.md).
- `sim_data_types_extended.rs` adds independent wide arithmetic, signed div/mod,
  packed layouts (including all-bit/mixed-state packed structs and
  multidimensional packed-bit arrays), net resolution, and backend-boundary
  fixtures, including acceptance at 1,048,575 bits and explicit rejection at
  1,048,576 bits. Packed unions and unpacked aggregate/member contexts remain
  outside the documented support claim.
- `sim_data_type_edges.rs` exercises signed/unsigned indices, real conversions,
  two-state subprogram and aggregate storage, enum defaults, numeric size-cast
  provenance, and near-limit storage combined with recursion. Its HDL lives in
  `fixtures/sim/data_type_edges/`; enum base-state/signedness and numeric
  source-enabled/source-less cast paths are covered at the exercised widths.
  Ambiguous source-less cast provenance must explicitly diagnose rather than
  silently change the cast width/sign. Test authors derive oracles from the official local
  `docs/specification/` files without reading production implementation code.
- `sim_data_types_next.rs` is the bounded next-phase inventory for packed
  and unpacked unions/structs, streaming and `inside`, static subprogram
  storage, strings, chandles, dynamic/associative arrays, and queues. Its
  focused coverage totals 20 positive cases plus one vector-strength rejection
  and one explicit unsupported nonconstant-static-initializer case. Runtime-
  dependent static initializers are rejected rather than evaluated on first
  call. This bounded inventory is not an exhaustive conformance claim.
- `elab_resolve.rs` exercises `core::elab`; `config_effect.rs` observes
  configured `-D` ifdef/elsif selection and top-level `-P` parameter-driven
  generate branches through the owned `DesignModel`.
- `elaboration/run_elab_check.sh` is the elaboration regression suite
  (runs `elab_check` over the test designs; ref binding must stay 100% and
  resolved parameter values must match the expected outputs).
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
- `emit_decoupling.rs` pins the pipeline shape with architectural
  greps: `sim::emit_c` consumes only IR types (no `core::db`/`ffi`/`vpi`/
  `unsafe`/`VpiHandle`), and `sim::codegen` builds an `IrModel` instead of
  emitting runtime C calls directly.
- `sim_opt_differential.rs` runs designs twice — once with
  `OptConfig::default()` (all passes) and once with `OptConfig::none()` —
  building both models via `sim::build::build_model_cmake` and asserting
  byte-identical stdout.
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
  `sim_delay.rs` covers the source-recovered constant delay subset and its
  explicit rejection boundaries.
  `sim_time_literals.rs` checks fixed-point/unit-suffixed/scientific and real
  parameter delays, lexical shadowing, and local rounding before global
  scheduling, with optimizer parity. `sim_time_values.rs` covers owned-source
  time-literal recovery, module-unit realtime values and rejection boundaries.
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

[ci.yml](../.github/workflows/ci.yml) runs on Ubuntu. Before release, run its
complete serialized `lint` gate:

```sh
cargo fmt --check
cargo check --all-targets --all-features
cargo check --lib --no-default-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features -- --test-threads=1
```

The PR/manual `generated-runtime-sanitizers` job has a 180-minute limit and
runs `runtime_values`, `runtime_boundaries`, `sim_counter`, `sim_data_types`,
`sim_data_types_next`, `sim_function`, and `sim_loops` with GCC ASan/UBSan.
This checks
generated C/runtime memory safety, not LSP admission. The 15-minute
`dependency-audit` job runs `cargo audit` on those triggers and Mondays at
04:17 UTC. Neither uploads reports; workflow logs are evidence.

[build-binaries.yml](../.github/workflows/build-binaries.yml) produces release
binaries on tags/manual dispatch. Windows/macOS legs remain placeholders/
untested: the root native pipeline is validated only on x86_64-linux-musl.
Keep platform claims aligned with local `persistence/platforms.md` evidence.
