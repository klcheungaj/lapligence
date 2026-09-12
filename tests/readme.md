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

### Coverage

- [Datatype/net matrices](fixtures/sim/type_conformance/readme.md): mixed operators, resolution truth tables, casts, two/four-state storage and X/Z-to-zero conversion.
- [Feature regressions](fixtures/sim/partial_features/readme.md): ports, events, timing, packed selections, real sensitivity/math, gated host commands, and resumable `$stop` control.
- [Process semantic regressions](fixtures/sim/process_semantics/readme.md):
  always-family sensitivity, time-zero execution, writer/timing contracts and
  legal latch/flip-flop controls.
- [Physical-time regressions](fixtures/sim/physical_time/readme.md): 1fs–100s
  scheduling, checked overflow, and femtosecond waveform timestamps.
- [Waveform regressions](fixtures/sim/waveform/readme.md): file-backed VCD/FST
  catalogs, `$dumpvars` depth/name filtering, aliases, declared array indices,
  value types and dump lifecycle controls.
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
- `sim_opt_differential`: optimized/unoptimized equivalence against regression traces.

### Limits of the tests

- Passing fixtures establish exercised behavior, not complete IEEE conformance.
- Wide probes cover representative operations at 65,536 and 1,048,575 bits; they do not exhaust every value or context.
- Capacity tests check rejection at 1,048,576 bits; fixed-size atoms retain their specified widths.
- Driver-boundary tests cover the 16-site resolved-net limit.
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
- CMake and a C compiler; the file-based conformance suites require both.
- Run commands from the repository root.

### Focused simulator suites

```sh
cargo test --locked --test sim_type_conformance --test sim_partial_features -- --test-threads=1
cargo test --locked --test sim_data_types --test sim_data_types_extended --test sim_data_type_edges -- --test-threads=1
cargo test --locked --test sim_data_types_next --test sim_data_types_completion --test sim_net_resolution --test sim_net_defaults --test runtime_values -- --test-threads=1
```

```sh
cargo test --locked --test sim_physical_time -- --test-threads=1
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
cargo test --locked --test sim_partial_features --test sim_type_conformance --test sim_procedural_assign --test runtime_values -- --test-threads=1
```

### Repository gate

```sh
cargo fmt --check
cargo check --locked --all-targets --all-features
cargo check --locked --lib --no-default-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features -- --test-threads=1
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
