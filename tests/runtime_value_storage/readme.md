# Dynamic packed storage checks

This standalone C11 test project exercises the `sv4_storage_t` ownership
building block and its waveform-snapshot consumer. It does not require Rust,
Slang, generated HDL, or libaco. The Cargo test `runtime_value_storage.rs` is
an optional wrapper around the same CMake/CTest project.

## Run

From the repository root:

```sh
cmake -S tests/runtime_value_storage -B target/storage-tests -DCMAKE_BUILD_TYPE=Debug
cmake --build target/storage-tests --config Debug
ctest --test-dir target/storage-tests --build-config Debug --output-on-failure
```

The complete suite needs CMake, a C11 compiler, threads and zlib. On Windows,
run from a configured native compiler environment and supply the zlib location
to CMake when needed. To run only the allocator/copy/move tests without zlib:

```sh
cmake -S tests/runtime_value_storage -B target/storage-core -DLLG_STORAGE_TEST_WAVEFORMS=OFF
cmake --build target/storage-core --config Debug
ctest --test-dir target/storage-core --build-config Debug --output-on-failure
```

For GCC/Clang AddressSanitizer and UndefinedBehaviorSanitizer, use a separate
build directory and add `-DLLG_STORAGE_TEST_SANITIZERS=ON` when configuring.
Enable leak detection using the sanitizer environment supported by the host.
For example, on Linux:

```sh
cmake -S tests/runtime_value_storage -B target/storage-asan -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DLLG_STORAGE_TEST_SANITIZERS=ON
cmake --build target/storage-asan --config Debug
ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 ctest --test-dir target/storage-asan --build-config Debug --output-on-failure
```

Configure another directory with `-DLLG_STORAGE_TEST_MODEL_WIDTH=65536` to
check that the waveform event descriptor stays small even when the legacy
source-value ABI has a large capacity. This setting must be at least 65 for
the waveform fixture. The core probe always builds the legacy header with a
one-bit model cap while testing dynamic allocations up to 1,048,575 bits.

## Coverage and limitations

`storage_probe.c` instruments malloc/free around the production storage fragment
without adding test hooks to the runtime API. It checks exact requested bytes,
zero-fill, contiguous planes, high-limb masking, clone independence, replacement,
self-copy/self-move, repeated destruction, NULL planes, zero width, the exclusive
supported bound, and 10,000 replacement cycles. Fault-injected child processes
must diagnose invalid widths and allocation failure, including allocation
failure before an existing destination is released. Those deliberately fatal
tests may print an abort result; the CMake wrapper checks the expected diagnostic.

`waveform_snapshot_probe.c` includes the production writer to test private queue
moves deterministically before starting threads. It fills and drains the entire
ring three times, mutates source storage before any captured event is consumed,
and checks moved-from slots. Public-API tests then exercise VCD/FST values,
X/Z and 65-bit payloads, wider registered views, an empty source value, aliases,
flush, thousands of pending events, dump limits, output errors, close without
flush, repeated initialization and idempotent close. The official FST reader
checks actual captured values. `storage_runtime.c` counts allocation and release
calls; counters are read only when no writer thread is live.

The writer fixture links only the storage portion of the value module, not
legacy arithmetic or the scheduler. This intentionally keeps its portability
and ownership coverage narrow. The main `sv4_t` representation and generated
model-width contract remain unchanged at this migration stage. Full generated
simulation, expression cleanup and coroutine cancellation are later gates.

New storage/writer code uses C11 without compiler extensions. Native macOS and
Windows builds must be validated on those hosts. The unchanged arithmetic
implementation still contains compiler-specific integer operations/intrinsics,
and the broader generated simulator has independent libaco platform constraints;
this suite does not certify them.
