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

Public C model emission/build is deliberately paused until P05 implements owned
initialization, temporary cleanup and activation/cancellation lifetimes. The old
runtime/wave selftests are explicitly fenced because their nested temporary and
static-initializer assumptions are unsafe with owners. They must be migrated,
not silently run without leak checks. Full generated HDL, complete Rust/C parity,
native macOS/MSVC and performance benchmarks are still outstanding. A component
suite passing does not certify unexecuted paths or prove a whole-runtime speedup.
