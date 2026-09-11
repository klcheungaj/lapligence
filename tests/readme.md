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
- [Feature regressions](fixtures/sim/partial_features/readme.md): ports, events, timing, packed selections and math.
- [Datatype basics](fixtures/sim/data_types/), [wide values](fixtures/sim/data_types_extended/) and [edge cases](fixtures/sim/data_type_edges/): operator/state combinations, limb boundaries and capacity rejection.
- [Aggregates and containers](fixtures/sim/data_types_next/) and [completion cases](fixtures/sim/data_types_completion/): storage, methods and conversion boundaries.
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

## Running tests

### Prerequisites

- Rust toolchain and initialized vendored dependencies.
- CMake and a C compiler; the file-based conformance suites require both.
- Run commands from the repository root.

### Focused simulator suites

```sh
cargo test --test sim_type_conformance --test sim_partial_features -- --test-threads=1
cargo test --test sim_data_types --test sim_data_types_extended --test sim_data_type_edges -- --test-threads=1
cargo test --test sim_data_types_next --test sim_data_types_completion --test sim_net_resolution --test sim_net_defaults --test runtime_values -- --test-threads=1
```

### One readable fixture

```sh
cargo run --bin llg -- --top tb tests/fixtures/sim/type_conformance/uwire.sv
cargo run --bin llg -- --no-opt --top tb tests/fixtures/sim/type_conformance/uwire.sv
```

### Generated-runtime sanitizers (GCC)

```sh
LLG_CC=gcc \
LLG_CFLAGS='-DACO_USE_ASAN -fsanitize=address,undefined -fno-omit-frame-pointer -fno-sanitize-recover=all' \
ASAN_OPTIONS='detect_leaks=1:strict_string_checks=1' \
UBSAN_OPTIONS='print_stacktrace=1:halt_on_error=1' \
cargo test --test sim_partial_features --test sim_type_conformance --test runtime_values -- --test-threads=1
```

### Repository gate

```sh
cargo fmt --check
cargo check --all-targets --all-features
cargo check --lib --no-default-features
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features -- --test-threads=1
```

- [CI workflow](../.github/workflows/ci.yml): full suite selection, sanitizer settings and release checks.
