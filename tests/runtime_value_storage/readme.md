# Dynamic runtime ownership checks

This standalone C11 project tests the production dynamic `sv4_t`, containers,
retained scheduler state, VPI, waveform ownership and cancellation-safe value
scopes. It does not need Rust, Slang or generated HDL. The optional Cargo wrapper
`tests/runtime_value_storage.rs` invokes the same CMake/CTest project.

## Native run

```sh
cmake -S tests/runtime_value_storage -B target/storage-tests -DCMAKE_BUILD_TYPE=Debug
cmake --build target/storage-tests --config Debug
ctest --test-dir target/storage-tests --build-config Debug --output-on-failure
```

CMake, a C11 compiler, threads and zlib are required for the full suite. Scheduler,
VPI and actual coroutine probes use vendored libaco on supported Unix x86 hosts;
other hosts explicitly skip these probes. Native Windows/macOS validation is not
implied by Linux results. Configure from a native compiler environment on Windows
and supply zlib's location if needed.

To run value/storage/container tests without waveform or scheduler dependencies:

```sh
cmake -S tests/runtime_value_storage -B target/storage-core -DLLG_STORAGE_TEST_WAVEFORMS=OFF -DLLG_STORAGE_TEST_SCHEDULER=OFF
cmake --build target/storage-core --config Debug
ctest --test-dir target/storage-core --build-config Debug --output-on-failure
```

The tests never define a model-width capacity. Legal narrow and very wide values
coexist using the same dynamic ABI. `LLG_STORAGE_TEST_MODEL_WIDTH` from stage 01
is removed and must not be passed to CMake.

## Sanitizers

```sh
cmake -S tests/runtime_value_storage -B target/storage-asan -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DLLG_STORAGE_TEST_SANITIZERS=ON
cmake --build target/storage-asan --config Debug
ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 ctest --test-dir target/storage-asan --build-config Debug --output-on-failure
```

The real coroutine stack-switch test is intentionally omitted under sanitizers:
vendored libaco does not advertise sanitizer fiber-switch integration. The
scheduler ownership probe tests private capture/commit/cancel paths without
switching the C stack, and the non-sanitized coroutine probe separately tests
real suspension. Never equate these two checks with ASan coverage of coroutine
stack switching.

## Coverage

| Probe | Checks |
| --- | --- |
| `storage_probe.c` | Exact allocation size, contiguous planes, top masking, independent clones, replacement/self-copy/self-move, repeated destruction, 10,000 replacements, zero width, exclusive maximum, and failure-atomic OOM replacement. Fatal child-process cases require the expected diagnostic. |
| `value_ownership_probe.c` | Independent results/input nonmutation across binary/unary operations and widths 0..257, selected self-alias writes, max-width construction/addition, conversions and string roundtrips. Allocation counters check steady live owners after each cycle. |
| `container_ownership_probe.c` | Recursive copies, alias-safe replacement, queue shifts/pops, pinned detached refs, wildcard associative key normalization, defaults, repeated replacements and teardown. |
| `scheduler_ownership_probe.c` | NBA capture/commit/masked cancellation, scopes, frames, inertial replacement, force baselines, sequence locals/endpoints, sampling, mailbox transfers, cleanup/reinit; wide typed formatting, long leading-zero plusargs and exact time-scaling vectors. |
| `coroutine_ownership_probe.c` | Registered values survive 1,000 real yields; completion and suspended-process cancellation unwind scopes, repeated 50 times. Native-only; no sanitizer stack-switch claim. |
| `vpi_ownership_probe.c` | Repeated 129-bit X/Z vector puts/gets, 65,537-bit text scratch, repeated function-result replacement, cached call cleanup and ten reinitializations. |
| `waveform_snapshot_probe.c` | Full-ring wrap and move clearing, mutation-after-capture, VCD/FST contents, X/Z, wide registered views, error/ignored events, pending-close and reinit. Event descriptors no longer embed fixed path storage. |

`tracked_value.c` instruments allocation calls in the full production value
facade without changing the public API. Counters distinguish live payload bytes
from RSS and verify no accumulation in tested loops. Sanitizers additionally
check allocations made by containers/scheduler/writer. Waveform storage counters
are atomic; final allocation/release checks run after joining the writer.

## Independent arithmetic oracle and optional legacy comparison

Build a shared value library without sanitizers for Python ctypes:

```sh
cmake -S tests/runtime_value_storage -B target/storage-oracle -DLLG_STORAGE_TEST_ORACLE_LIBRARY=ON
cmake --build target/storage-oracle --target value_oracle --config Debug
python tests/runtime_value_storage/value_oracle.py --dynamic target/storage-oracle/libvalue_oracle.so
```

Select the actual `.dylib`/`.dll` path for a different platform/configuration.
The script checks 35,000 results against Python arbitrary-precision integers.
The optional legacy comparison covers 84,600 value results plus real/decimal
conversions and input nonmutation. It requires the **stage-01** library, not the
new dynamic library, built with `LLG_MODEL_MAX_WIDTH=1024`. For example, on Linux:

```sh
cc -std=c11 -O2 -fPIC -shared -DLLG_MODEL_MAX_WIDTH=1024 /path/to/stage01/src/sim/rt/llg_value.c -lm -o /tmp/value-stage01.so
python tests/runtime_value_storage/value_oracle.py --dynamic target/storage-oracle/libvalue_oracle.so --legacy /tmp/value-stage01.so
```

The legacy option is test-only; production has no compatibility representation.
Both sides can share an old semantic bug, so the independent integer checks are
separate. This is not the repository's Rust/C parity gate.

## Integration boundary

The structured numeric whole-model path has owned initialization and cleanup,
but unmigrated features and legacy fragment APIs still reject emission. The old
runtime/wave selftests are explicitly fenced because their nested temporary and
static-initializer assumptions are unsafe with owners. They must be migrated,
not silently run without leak checks. Full generated HDL, complete Rust/C parity,
native macOS/MSVC and performance benchmarks are still outstanding. A component
suite passing does not certify unexecuted paths or prove a whole-runtime speedup.


## P05/P06 increment

`generated_scopes_probe.c` and `generated_coroutine_probe.c` are **hand-authored
C output-shape probes**, not files produced by running the Rust emitter. They
exercise registered expression scopes, short-lived cells retained by delayed
NBA/clocking writes, clocking-to-NBA transfer, distinct repeated lexical cells,
selected net masks/regions, recursive ownership patterns, yielding calls,
process completion, nonreturning finish, cancellation and stop/resume/close.
Allocation counters cover packed payloads; leak sanitizers additionally cover
ordinary heap allocations in the non-fiber suite.

Actual coroutine switching is native-only until libaco advertises sanitizer
fiber-switch hooks. The sanitizer configuration excludes both coroutine probes;
it still tests production scheduler queue/cleanup paths without stack switches.
The C test suite does not compile or execute the new Rust emitter.

New Rust tests are in `src/sim/emit_c/owned/tests.rs`. The ignored execution test
renders a numeric `ExecutionModel`, builds its C and checks a 1,000-iteration
result. It is not a frontend/HDL integration test. These tests and the new build
ABI/cache tests were not run in the delivery environment (no Rust toolchain).

```sh
cargo test --lib --no-default-features sim::emit_c::owned::tests
cargo test --lib --no-default-features structured_owned_model_executes_numeric_loop -- --ignored
```

The structured renderer keeps feature-specific guards until captured objects,
callbacks and other outstanding ownership paths are implemented; see its
[coverage matrix](../../src/sim/emit_c/owned/readme.md). Do not treat the old
fenced runtime fixtures or unexecuted legacy golden tests as passing this ABI.

The following standard-library Python helper verifies private fragment embedding
order, compiles the three flat runtime translation units in strict C11, and
checks current/stale ABI assertions. It supports GCC/Clang and MSVC-style
drivers; using it on Linux does not establish native MSVC compatibility.

```sh
python tests/runtime_value_storage/check_flat_runtime.py --compiler gcc --compiler clang
```


## P07 repeatable validation and measurements

From the project root, run all available GCC/Clang native configurations and
sanitizer-safe components:

```sh
python3 tests/runtime_value_storage/validate.py --compiler gcc --compiler clang --sanitizers
```

The default native configurations are **Debug and Release C builds**, not the
HDL optimizer modes. The script creates a fresh `target/p07/<run>/` evidence
directory with `report.json`, command logs and capability manifests. An explicit
`--output` must be new or empty; it will not overwrite old evidence. A zero-test,
missing-test, duplicate-test, disabled-test or stale-product result is rejected.
Command timeouts terminate their process groups on POSIX or process trees on
Windows. Python 3.10 or later and CMake/CTest are required.

Add `--full` to require Cargo/Rust formatting and checking, structured emitter
unit tests, ABI/cache tests, both ignored Rust-emitted C tests (numeric loop and
16 start/advance/close cycles with a maximum-legal-width global), the public HDL
ownership suite in both optimizer modes, and the full all-feature repository
suite. Missing Rust is **blocked**, not passed; a successful Cargo command with
zero executed tests is also rejected. Exit codes are 0 for requested checks
passing, 1 for failures, and 2 for blocked prerequisites. The independent
integer oracle refuses Python optimization modes that would disable its checks. The report remains
host-scoped: it never certifies other operating systems or arbitrary coroutine
sanitizer support.

`--without-waveforms` and `--without-scheduler` are explicit component-only
exclusions. CMake also records native libaco restrictions. The Windows MSVC CI
lane deliberately covers values/containers only, without waveform or scheduler
claims. The macOS lane records the actual architecture-dependent scheduler
coverage. These CI jobs are configurations, not executed platform evidence.
Full-host mode treats missing native waveform/scheduler/coroutine coverage as
blocked. Its generated HDL execution is native, not an implicit sanitizer run.

### New probes

- `four_state_probe.c`: independent scalar four-state truth tables, exhaustive
  one-bit states/signedness, seeded mixed-width values, X/Z conditionals,
  equality, casts/resizing, aliasing selected writes, signed index extremes and
  operand-independence checks. It runs under the component sanitizers as well.
- `value_lifetime_benchmark.c`: dynamically allocated mixed-width descriptors,
  one 1,048,575-bit sentinel, and repeated clone/replace/add/resize/move cycles.
  Every completed cycle must return to the exact live-allocation/payload-byte
  baseline; teardown must reach zero. JSON includes peak bytes, counts, CPU
  time and checksum. The runner measures elapsed process time separately.
- `test_validation_runner.py`: tests the runner's inventory, measurement,
  process failure/timeout and artifact-selection checks.

The tracked allocator measures allocations **inside the value implementation**,
including its scratch allocations. It does not measure allocator metadata,
whole-process RSS, all scheduler/container allocations or frontend memory.
Sanitizers separately cover the exercised component lifecycles. The benchmark's
`fixed_capacity_reference_bytes` is an analytical old-layout payload reference,
not a measured run of the legacy simulator. CPU/elapsed numbers are observations
for this value workload, not evidence of a whole-simulator speedup.

```sh
python3 -m unittest discover -s tests/runtime_value_storage -p test_validation_runner.py -v
python3 tests/runtime_value_storage/check_flat_runtime.py --compiler gcc --compiler clang
```

The flat-source checker also accepts `cl`/`clang-cl` and
`--without-scheduler` for portable value/container-only compilation. It first
requires the current ABI to compile, then requires the stale ABI to fail.
