# Tests

## Scope and layout

- Shared Slang frontend, owned semantic database, simulator and LSP integration.
- [Shared harnesses](support/readme.md): temporary directories, process cleanup and timeouts.
- [LSP fixtures](fixtures/lsp/): framed stdio tests, manifests and declared source headers.
- [Simulation feature status](../docs/sim_features.md): the sole support checklist.

## Simulator testing methodology

- Target IEEE 1364-2001 and IEEE 1800-2009 using the local [specifications](../docs/specification/).
- Keep end-to-end designs in checked-in `.v` / `.sv` files; pass them directly to `llg`.
- Run each conformance fixture with default optimization and `--no-opt`.
- Keep independent expected results in Rust: explicit truth tables, bit strings and width/signedness arithmetic.
- Compare exact stdout, expected diagnostics and exit status; reject unexpected lowering warnings.
- Test invalid syntax/unsupported contexts separately from successful execution.
- Isolate child working directories; serialize and restore any parent CWD changes.
- Use focused in-memory sources for frontend, database and IR unit tests.
- Run generated C under GCC ASan/UBSan; sanitizer coverage does not instrument the vendored Slang archive.

### Fixture integrity before a native build

Run `python3 scripts/check_sim_fixture_integrity.py --tracked` after staging every
new fixture. The gate checks the known static harness shapes, not arbitrary Rust
expressions. Its unit tests run with
`python3 -m unittest discover -s scripts -p test_sim_fixture_integrity.py`.
CI runs both before the native build. Missing files and files absent from Git's
index are errors; do not silently skip their tests.

The Group 1 repair supplies 51 newly authored replacements for missing HDL
inputs in the delivered topic suites. They preserve those suites' independent
value/diagnostic expectations, except where this repair deliberately converts
R14's legal packed-input/ref rejection cases into positive tests. The original
missing contents and six advertised-but-absent suites were not recovered.
Their historical pass counts are not part of the current acceptance record.

### Coverage

- `sim_group1_formal_repairs`: R09/R14 packed activation isolation, recursion,
  callbacks, member state conversion, immediate references, captured copy-out
  addresses and preserved const/NBA negatives. `sim_edition` exercises the shared
  execution/navigation edition policy, macro/directive context, standard timing
  checks and explicit registered extensions. The new tests are unexecuted until
  run in the pinned native/Rust environment.

- `sim_group1_repairs`: file-backed callback, instance-index, selected aggregate
  and fixed-streaming regressions for the first Group 1 review repair batch.
  Runs both optimizer modes and uses its own `group1_repairs` fixture directory. Added tests are not acceptance evidence
  until executed on the target toolchain.

- [Datatype/net matrices](fixtures/sim/type_conformance/readme.md): mixed operators, resolution truth tables, casts, two/four-state storage and X/Z-to-zero conversion.
- [Feature regressions](fixtures/sim/partial_features/readme.md): ports, events, timing, packed selections, real sensitivity/math, time formatting, immediate and deferred four-state assertions, gated host commands, and resumable `$stop` control.
- [Concurrent assertion regressions](fixtures/sim/concurrent_assertions/): Preponed sampling, asynchronous `disable iff`, overlapping attempts, Reactive actions, vacuity accounting, and end-of-simulation pending-attempt handling in both optimizer modes.
- [Process semantic regressions](fixtures/sim/process_semantics/readme.md):
  always-family sensitivity, time-zero execution, writer/timing contracts and
  legal latch/flip-flop controls.
- `sim_process_control`: process-class identity/status observations,
  suspended waits, terminal awaits, recursive kill cleanup and independent
  delayed NBA ownership in both optimizer modes.
- `sim_semaphore`: zero-key construction and exact `try_get` results,
  differing-count FIFO contention, suspended wake deferral, task-handle
  arguments, and cancellation-safe blocked waiters in both optimizer modes.
- `sim_mailboxes`: typed/untyped bounded and unbounded mailbox FIFO order,
  peek/try APIs, packed/real/string/handle copy semantics, waiter handoff and
  process-kill cleanup in both optimizer modes.
- [Physical-time regressions](fixtures/sim/physical_time/readme.md): 1fs–100s
  scheduling, checked overflow, and femtosecond waveform timestamps.
- [Waveform regressions](fixtures/sim/waveform/readme.md): file-backed VCD/FST
  catalogs, `$dumpvars` depth/name filtering, aliases, declared array indices,
  value types and dump lifecycle controls.
- [File-I/O regressions](fixtures/sim/file_io/readme.md): owned descriptor
  masks, multichannel output, deferred file formatting, portable seek/rewind/
  flush status, descriptor-table boundaries, HDL-aware formatted/character/
  line input, and packed/ascending/descending binary reads.
- `sim_memory`: fixed packed-memory `$readmemh/$readmemb` parsing,
  `$writememh/$writememb` roundtrips, range/order/address handling, four-state
  conversion and file-size diagnostics in optimized and unoptimized models.
- [Procedural assignment regressions](fixtures/sim/procedural_assign/): PCA priority, replacement, dependencies and force layering.
- `sim_reference_args`: typed `ref`/`const ref` aliasing, selected actuals, nested calls, recursion and suspension observation.
- [Datatype basics](fixtures/sim/data_types/), [wide values](fixtures/sim/data_types_extended/) and [edge cases](fixtures/sim/data_type_edges/): operator/state combinations, limb boundaries and capacity rejection.
- [Aggregates and containers](fixtures/sim/data_types_next/) and [completion cases](fixtures/sim/data_types_completion/): storage, methods and conversion boundaries.
- [Logical expression regressions](fixtures/sim/logical_ops/): ordinary `->`/`<->` four-state truth tables, precedence, side-effect evaluation, real operands and optimizer parity.
- `sim_stochastic`, `runtime_stochastic`: IEEE stochastic-analysis queue order, status codes, simulation-time statistics and scheduler-independent runtime boundaries.
- [Executable-node coverage](fixtures/sim/u01_coverage/): source-located fail-closed unsupported nodes, compile-time declarations and elaborated-away branches.
- `sim_edition`: checked-in 2009 time-literal rounding and 2001 edition/keyword CLI probes.
- `sim_physical_time`: file-backed 1fs/10fs/100fs/1ps/1ns mixed scopes,
  10s/100s units, checked overflow rejection, and VCD femtosecond
  headers/timestamps.
- `runtime_values`, `runtime_boundaries`, `region_conformance`: standalone C value checks, resource bounds and scheduling order.
- [Dynamic packed storage](runtime_value_storage/readme.md): exact allocation
  sizes, deep copies, moves, replacement, width boundaries, injected allocation
  failure, and waveform snapshot transfer/cleanup. Runs directly through CMake
  without Cargo/Slang, or through `runtime_value_storage.rs`.
- `sim_random`, `runtime_random`: legacy `$random`/`$dist_*` Annex N vectors
  through generated models and standalone C runtime boundary checks, each at
  optimized and unoptimized levels.
- `sim_opt_differential`: optimized/unoptimized equivalence against regression traces.

### Limits of the tests

- Passing fixtures establish exercised behavior, not complete IEEE conformance.
- Wide probes cover representative operations at 65,536 and 1,048,575 bits; they do not exhaust every value or context.
- Capacity tests check rejection at 1,048,576 bits; fixed-size atoms retain their specified widths.
- Driver-boundary tests exercise registry growth past the retired per-net ceilings.
- Two-state net declarations are language errors (§1800-2009 6.7); two-state conversion tests apply to variables and expressions.
- Platform build configuration alone is not evidence of successful native execution.
- Feature restrictions and implementation limits are maintained in [sim_features.md](../docs/sim_features.md).
- Zero-time execution uses `LLG_ZERO_LOOP_LIMIT` for scheduler passes and
  `LLG_PROCESS_STEP_LIMIT` for generated process back-edges (with
  `LLG_NONCONVERGENCE_LIMIT` as an alias). Both default to 10,000,000, require
  a positive decimal `uint64_t`, and report invalid or overflowing values
  before execution; setting only the scheduler variable also applies that
  value to the process budget.

## Running tests

### Prerequisites

- Rust 1.98.0 toolchain and initialized vendored dependencies.
- `cargo-nextest` (`cargo install cargo-nextest --locked`).
- CMake and a C compiler; the file-based conformance suites require both.
- Run commands from the repository root.
- Generated-runtime archives are shared under `target/llg-runtime-cache` by
  default; set `LLG_RUNTIME_CACHE_DIR` to override the location. Relative
  override paths are resolved from the repository root.
- Nextest runs 8 tests concurrently by default; use
  `--profile max-threads` to opt in to 32 on a sufficiently large host.

### Focused simulator suites

```sh
cargo nextest run --locked --test sim_type_conformance --test sim_partial_features
cargo nextest run --locked --test sim_data_types --test sim_data_types_extended --test sim_data_type_edges
cargo nextest run --locked --test sim_data_types_next --test sim_data_types_completion --test sim_net_resolution --test sim_net_defaults --test runtime_values --test runtime_random
```

```sh
cargo nextest run --locked --test sim_physical_time
cargo nextest run --locked --test sim_mailboxes
```

### One readable fixture

```sh
cargo run --locked --bin llg -- --top tb tests/fixtures/sim/type_conformance/uwire.sv
cargo run --locked --bin llg -- --no-opt --top tb tests/fixtures/sim/type_conformance/uwire.sv
```

### Generated-runtime sanitizers (GCC)

```sh
LLG_CC=gcc \
LLG_CFLAGS='-DACO_USE_ASAN -fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all' \
ASAN_OPTIONS='detect_leaks=1:strict_string_checks=1' \
UBSAN_OPTIONS='print_stacktrace=1:halt_on_error=1' \
cargo nextest run --locked --test sim_partial_features --test sim_type_conformance --test sim_procedural_assign --test runtime_values --test runtime_random
```

### Repository gate

```sh
cargo fmt --check
cargo check --locked --all-targets --all-features
cargo check --locked --lib --no-default-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo nextest run --locked --all-features
cargo test --locked --doc --all-features
```

- [CI workflow](../.github/workflows/ci.yml): full suite selection, sanitizer settings and release checks.

### Reproducible baseline and per-patch regression

The exact Rust toolchain is pinned in [`rust-toolchain.toml`](../rust-toolchain.toml),
and Cargo dependencies are resolved by [`Cargo.lock`](../Cargo.lock). The
vendored Slang and libaco gitlinks must be checked out at the upstream base
revisions recorded by the root commit; `build.rs` applies the reviewable patches
under `patches/` before native sources are consumed. No project-specific vendor
commits are allowed. `scripts/run-regression.sh` verifies the gitlinks before
running the serialized gate and records provenance, command lines, phase status,
timings, and complete logs in ignored `persistence/` output.

Run the gate in separate directories before and after a feature patch, then
compare the stable phase summary:

```sh
scripts/run-regression.sh --label before --output-dir persistence/u05/before
scripts/run-regression.sh --label after --output-dir persistence/u05/after
diff -u persistence/u05/before/summary.tsv persistence/u05/after/summary.tsv
diff -u persistence/u05/before/test-inventory.log persistence/u05/after/test-inventory.log
```

The runner refuses tracked root or submodule modifications by default. A local
run may pass `--allow-dirty-root` or `--allow-dirty-submodules`, but its
`metadata.tsv` is marked `reproducible=no` and must not be used as clean
baseline evidence. Expected stdout and diagnostic assertions remain in the
Rust tests; this workflow never blesses changed expected output. The
`runtime_values` phase is a standalone C value-runtime check, while the full
suite and generated-runtime matrix are the evidence for simulator execution.
The inventory diff makes added, removed, or renamed tests visible alongside
the phase-status comparison; any expected-output change still requires a
feature/clause justification in the patch.

## Priority-review regression sources

- `sim_process_control::a_child_killing_its_ancestor_never_returns_to_released_locals`
  covers ancestor kill with live automatic locals and nested descendants.
- The `semaphore/cancel_{head,tree,named,fork}.sv` fixtures distinguish live
  FIFO reservice from mid-cancellation grants and cover the public cancellation
  entrypoints. No extra `put` is used to make the waiting request runnable.
- `sim_vpi` adds vector-ownership and callback-borrow-lifetime plugins. They
  exercise zeroed, format-only, poisoned and caller-buffer vector requests,
  multiword X/Z results, and stale call/argument/iterator handles across all
  three system-task callback kinds. Permanent model handles remain valid.
- `sim_dpi::dpi_string_results_are_snapshotted_before_aliased_copyout` covers
  an inout echo, swapped buffers, a void return, and shared output/return
  pointers. The C emitter has a separate ordering regression for string,
  integral and void returns.

These regression sources were added by static review; their addition is not
recorded test-execution evidence.


## Follow-up review regression sources

The supplied `test-to-be-added` sources now live under
`fixtures/sim/imported_probes/`. `sim_imported_probes.rs` actively checks five
additional acceptance witnesses in both optimizer modes. Eight other supplied
acceptance inputs already have active feature-suite equivalents; net alias
connectivity is retained as an ignored test. The review counterexamples are
preserved with a suite mapping in their README, including the C integration
fragments that cannot run as standalone designs.

`sim_review_batch2.rs` contains origin-specific program exit/completion cases,
postponed alias reads, delayed-alias event/level waiters, scalar mailbox mismatch
and FIFO cases, and independent assertion-failure/severity accounting. The
program fixtures use multiple instances of the same definition and multiple
initials, with a module-defined task invoked from both program and module
origins. Mailbox mismatch tests preserve the queued message and the target;
nominal enum/handle type equivalence is exercised separately by the batch-04 source regressions.

`runtime_rng.rs` checks exact parent seed consumption, state replay and the
independence of already-created children. Prior draws in a parent are allowed
(and required) to affect a subsequently created child's seed. Existing program
and mailbox expectations are updated to those specified contracts rather than
weakening stdout/stderr checks. These additions and changes were inspected
statically only; no build or test execution is claimed.


## Ten-finding review batch 03

`sim_review_batch3.rs` supplies focused cases for Observed clocking-block
publication, inheritance-layer construction, factory receiver binding,
non-packed property initializers, assertion off/kill and property truth,
numeric-prefix scanning, and the FD/MCD bit contract. The dedicated VPI suite
adds requested time-format/scaling coverage; `runtime_review_batch3.rs` uses
two different sized-function call descriptors so frontend argument coercions
cannot conceal shared return-width state. Existing I/O oracles now distinguish
FDs from MCDs instead of asserting implementation-assigned slot numbers.

These test sources were added by static inspection only. No new passing-test
result is recorded, and shared stdout/stderr comparisons are unchanged.


## Review batch 04

- `sim_review_batch4.rs` supplies unexecuted regressions for retained outdated
  queue refs (including aliases, self-assignment, suspension and cancellation),
  nominal and structurally equivalent mailbox types, ref mailbox destinations,
  empty sequence boundaries, guarded repetition, nested/tied `first_match`,
  coincident/noncoincident multiclock boundaries and inherited clock flow,
  implication-local snapshots, and disjoint/overlapping packed-prefix analysis.
- `runtime_containers.rs` adds direct retained-cell lifetime assertions,
  including sharing, queue destruction, reorder identity, and final release.
- The H25 multiclock fixture now distinguishes a same-time destination from a
  strictly later edge; its expected output follows that distinction. The batch-03
  CLI harness imports the shared simulation helper required by `sim_cli`.
- No compiler, simulator, runtime probe, sanitizer, or Cargo command was run for
  these changes. Shared stdout/stderr comparisons remain unchanged.

## Source organization

`lsp_stdio.rs` retains the framed JSON-RPC client and shared process helpers;
`lsp_stdio/` contains responsibility-named suites. The integration-test crate
uses explicit `lsp_stdio/...` module paths so these suites are not discovered
as independent Cargo test targets. Existing checked-in HDL fixtures and
independent expected results remain with their original suite owners. The stdio
and feature-test cases now have domain-qualified names; update exact-name
filters to include that domain. Integration-test binary names are unchanged.

Runtime source-assembly unit tests in `src/sim/rt/tests.rs` compare the C facade
include order with the embedded flat implementation; they do not compile C or
replace runtime execution tests. See [the source map](../docs/source_layout.md)
for other unit-test and implementation domains.


## Dynamic ownership validation

- Run `python3 tests/runtime_value_storage/validate.py --compiler gcc --sanitizers`
  for native Debug/Release components, strict flat C/ABI checks, independent
  oracles and exact live-allocation plateau measurements.
  - Add `--full` to require Rust checks, both active emitted-model tests,
    public HDL cases in both optimizer modes, and the repository suite.
  - New or empty output directories retain `report.json` and actual command logs;
    missing prerequisites and excluded platform components never count as passes.
- [The component guide](runtime_value_storage/readme.md) describes portable and
  native-fiber coverage, environment requirements, exit codes and memory metrics.
- [HDL fixtures](fixtures/sim/dynamic_ownership/readme.md) are positive acceptance
  tests, not claims that their migration-dependent generated paths already pass.
- [Dynamic-owner CI](../.github/workflows/dynamic-owners.yml) configures scoped
  Linux, macOS and Windows component checks plus a manual Linux full-host gate.
  Reports and command logs are printed to workflow logs; it uploads no artifacts.
  CI configuration is not native-platform execution evidence.

The dynamic component inventory also builds the original runtime and waveform
self-tests with allocation accounting. Numeric vectors and waveform cleanup run
in the sanitizer-safe lane; complete scheduler/region/stop-resume/budget probes
use native coroutines. Callback-finish probes cover packed snapshot/result cleanup
and all-context adoption, including shared eval/condition frames. Exact-address
scope-index and event-array probes cover retained targets, index churn and invalid
handle waits without weakening existing assertions. The checked inventory must
include these tests when their scheduler/waveform prerequisites are enabled.

The positive HDL ownership fixtures include numeric inputs/defaults/inout copy-in,
owned captured forks, evaluated/filtered waits and indexed events. They run through
`llg` with both optimizer settings; source addition or a migration diagnostic is
not a passing result. Whole-model Rust emitter tests also verify shared-context
reference counts and ordered index destruction, rather than accepting detached
expression fragments as owners.

The standalone value, container and file-I/O Cargo probes share their C sources
with the component suite (`*_isolation_probe.c`), so component validation includes
the original assertions and per-vector ownership cleanup. Run these individually:

```sh
cargo test --locked --test runtime_values --test runtime_containers --test runtime_file_io
cargo test --locked --lib --no-default-features sim::emit_c::owned::tests
cargo test --locked --no-default-features --test sim_dynamic_ownership -- --test-threads=1
```

The active `owned/tests/nextest_regressions.rs` models exercise the real emitter's
cancellation-before-copyout, lexical activation exit, inertial, strobe and force
paths. `nextest_control_probe.c` checks similar runtime patterns without Rust;
its native pass does not substitute for those generated-model tests. The full
runner and main CI select the active tests without `--ignored`.

### Native ownership source-repair regressions

The native-value follow-up uses the existing public container/string/file/process
HDL suites without weakening their expected results. Nine structural tests in
`src/sim/emit_c/owned/tests/native_values.rs` check emitter ownership contracts.
The component guide documents `native_value_scopes` and `native_input_callbacks`;
these are runtime probes and must not be reported as Rust/HDL passes. Failure-target
manifests are delivery inventories, not the maintained language feature checklist.
