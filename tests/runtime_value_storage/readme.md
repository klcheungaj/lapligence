# Dynamic runtime ownership checks

This standalone C11/CMake project tests production `sv4_t`, containers, retained
scheduler state, VPI, waveforms and cancellation-safe scopes. It needs no Rust,
Slang or HDL; `tests/runtime_value_storage.rs` invokes the same CTest project.
All C probes here are handwritten, including those named `generated_*`.

## Native run

```sh
cmake -S tests/runtime_value_storage -B target/storage-tests -DCMAKE_BUILD_TYPE=Debug
cmake --build target/storage-tests --config Debug
ctest --test-dir target/storage-tests --build-config Debug --output-on-failure
```

Full coverage needs a C11 compiler, CMake, threads and zlib. Scheduler/VPI/fiber
probes use libaco on supported Unix x86 hosts; other hosts record exclusions.
Use a native compiler environment on Windows and supply zlib's location if needed.
For value/storage/container-only checks:

```sh
cmake -S tests/runtime_value_storage -B target/storage-core -DLLG_STORAGE_TEST_WAVEFORMS=OFF -DLLG_STORAGE_TEST_SCHEDULER=OFF
cmake --build target/storage-core --config Debug
ctest --test-dir target/storage-core --build-config Debug --output-on-failure
```

Do not pass the removed `LLG_STORAGE_TEST_MODEL_WIDTH`. Narrow/wide values share
the dynamic ABI, not a model-width allocation capacity.

## Sanitizers

```sh
cmake -S tests/runtime_value_storage -B target/storage-asan -DCMAKE_BUILD_TYPE=Debug -DCMAKE_C_COMPILER=clang -DLLG_STORAGE_TEST_SANITIZERS=ON
cmake --build target/storage-asan --config Debug
ASAN_OPTIONS=detect_leaks=1:halt_on_error=1 UBSAN_OPTIONS=halt_on_error=1 ctest --test-dir target/storage-asan --build-config Debug --output-on-failure
```

The component sanitizer lane excludes actual coroutine stack switching. Scheduler
capture/commit/cancel probes without stack switches and separate native fiber tests
do not establish sanitizer integration for shared stacks. Counts depend on enabled
capabilities; check the exact inventory rather than assuming a fixed count.

## Coverage

| Probe / group | Contract |
| --- | --- |
| `storage_probe.c` | Exact contiguous planes, masking, independent clone/copy/move, repeated destruction, 10,000 replacements, zero/exclusive widths and failure-atomic OOM. Fatal cases require specific diagnostics. |
| `stream_preflight_probe.c` | INT64 endpoints, declared bounds/traversal, unknown selectors, source-size rejection including later short segments, and zero remaining packed owners. |
| `array_conditional_probe.c` | Immediate-element equality/defaults versus packed mux, mixed X/zero defaults, aliased inputs, boundary/max widths, independent results, 10,000 replacements and malformed shapes. |
| `four_state_probe.c`, `value_ownership_probe.c` | Independent scalar state/sign tables, casez/casex, mixed widths through 4,097 bits, equality/mux/arithmetic/selections, widths 0..257, maximum values, conversion, nonmutation and steady ownership. |
| `container_ownership_probe.c` | Recursive copies, alias-safe replacement, queue shifts/pops/pinned detached refs, normalized associative keys/defaults and teardown. |
| `port_net_collapse_probe.c` | All seven runtime kinds at 1/65/129 bits, strengths/defaults/release, delayed alias/array publication and zero owners; no frontend policy or stack switching. |
| `scheduler_ownership_probe.c` | Captured/masked NBA, scopes/frames, inertial/force state, sequence locals/endpoints, sampling, mailbox cleanup, wide formatting, plusargs and exact time scaling. |
| `coroutine_ownership_probe.c` | 1,000 actual yields and completion/cancel unwind, repeated 50 times; native fiber lane only. |
| `vpi_ownership_probe.c` | 129-bit X/Z puts/gets, 65,537-bit text scratch, result replacement, call cleanup and ten reinitializations. |
| `waveform_snapshot_probe.c` | Ring wrap/move clearing, mutation-after-capture, VCD/FST values, wide views, ignored/error events and pending close/reinit. |
| `scope_index_probe.c`, `callback_finish_probe.c` | Growth/tombstones/out-of-order releases, retained cells, finish during evaluators, shared per-field contexts and force/qualifier cleanup. |
| `event_array_probe.c` | Mixed declared directions, negative/out-of-range/X selectors and inert invalid waits; select-only mode sanitizer-safe, waits native-fiber. |
| `nextest_control_probe.c` | Native cancellation, staged outputs, lexical activation exits, inertial/strobe/force callbacks and repeated starts. |
| `generated_scopes_probe.c`, `generated_coroutine_probe.c` | Handwritten output shapes: lexical cells, retained NBA/clocking transfers, masks, recursion, yielding calls, finish/cancel and stop/resume/close; coroutine mode native-only. |
| `native_ownership_probe.c` | Root string cleanup and independent copies; 15 input/callback modes repeated eight times, including scans/plusargs/containers and writer cancellation. Callback mode native-fiber. |
| `native_boundaries_probe.c` | Sanitizer-safe ref scope/relocation/removal; nine mailbox/stream modes repeated eight times with reentrant delivery, peek, cancellation and termination in native fibers. |
| `review_lifetimes_probe.c` | Sanitizer-safe native indexing, detached refs/tombstones/zero payloads and wide indices; native-only stable real/shortreal mailbox targets and publication cancellation. Defensive API effects are not legal read-only HDL callback claims. |
| `packed_selection_probe.c`, `packed_selection_scheduler_probe.c` | Independent per-bit oracle for 7,056 two-step chains plus third refinement, aliasing/X/wide indices, limb endpoints, NBA masks and synchronous scanner targets; scheduler cases do not switch stacks. |
| `packed_formal_probe.c` | 4,096 private-input mutations without caller changes, immediate ref publication, two-state member conversion and neighboring-field preservation; sanitizer-safe. |
| `fixed_array_reduction_probe.c` | Five folds, first-element X/Z seeding, signed/widened maps, nested values/declared indices and 65/129-bit owners; 200,000 separate eight-bit cells without flattening and exact cleanup. |
| `sequential_predicate_probe.c` | 64 truth/prefix combinations, 129-bit and real truth, packed/array merge differences; 10,000 ownership cycles, bounded peaks and zero skipped-clause allocations. |
| `memory_image_probe.c` | Eight exact-output modes at 1/7/8/65/129 bits, signedness, X/Z padding, numeric extension/truncation, original-width two-state enum overflow, sparse files and equivalent signed address spellings; no stack switches. |
| `value_lifetime_benchmark.c` | Mixed-width clone/replace/add/resize/move with a 1,048,575-bit sentinel; exact per-cycle baseline, bounded peak, zero teardown, counts/checksum/CPU time. |

Retain capability-specific entries such as `packed_selection_map`, zero/storage/
value rejection modes, `packed_selection_nba`, `packed_selection_input`,
`packed_formal_owner_contracts`, both fixed-fold/predicate modes and memory_image.
Do not silently omit tests or count native cases as accepted HDL features.

## Measurement and integration boundaries

`tracked_value.c` instruments the full production value facade, including scratch
allocations. Coherent counters use a C11 atomic flag because the writer may free
values; check final totals after joining it. Counts exclude allocator metadata,
RSS, frontend memory and uninstrumented scheduler/container/native allocations.
Sanitizers cover exercised heap lifecycles separately. The benchmark's
`fixed_capacity_reference_bytes` is analytical, not a measured legacy simulator;
CPU/elapsed measurements do not establish whole-simulator speedups.

Original value/container/file-I/O isolation probes are shared with Cargo. Original
runtime/waveform sources are built directly, not replaced with smaller expectations.
Test-only temporary-owner adapters are drained at boundaries and never become
production allocation policy. Callback returns remain independent owners;
consuming formatters get fresh arguments. Iterate actual limb counts.

Actual Rust emitter tests render `ExecutionModel`, build C and exercise ownership
and repeated start/close; public HDL tests add frontend/lowering. Keep all layers:

```sh
cargo test --lib --no-default-features sim::emit_c::owned::tests
cargo test --lib --no-default-features structured_owned_model_
cargo test --locked --no-default-features --test sim_dynamic_ownership
```

The public publication/cancel/plateau cases retain independent optimizer-paired
oracles; migration rejection fails. See [repository validation](../readme.md#dynamic-ownership-validation)
and [feature status](../../docs/sim_features.md#status-markers).

## Independent arithmetic oracle and optional legacy comparison

Build the ctypes library without sanitizers:

```sh
cmake -S tests/runtime_value_storage -B target/storage-oracle -DLLG_STORAGE_TEST_ORACLE_LIBRARY=ON
cmake --build target/storage-oracle --target value_oracle --config Debug
python tests/runtime_value_storage/value_oracle.py --dynamic target/storage-oracle/libvalue_oracle.so
```

Use the actual dylib/dll/configuration path on other hosts. The script checks
35,000 results against Python integers and rejects Python optimization that disables
checks. Optional legacy comparison checks 84,600 values plus real/decimal conversion
and input nonmutation, using the stage-01 library, not the dynamic library:

```sh
cc -std=c11 -O2 -fPIC -shared -DLLG_MODEL_MAX_WIDTH=1024 /path/to/stage01/src/sim/rt/llg_value.c -lm -o /tmp/value-stage01.so
python tests/runtime_value_storage/value_oracle.py --dynamic target/storage-oracle/libvalue_oracle.so --legacy /tmp/value-stage01.so
```

This test-only representation is not production compatibility or the Rust/C parity
gate. Independent integer checks remain necessary when both implementations share
an old semantic error.

## Repeatable validation

```sh
python3 tests/runtime_value_storage/validate.py --compiler gcc --compiler clang --sanitizers
python3 -m unittest discover -s tests/runtime_value_storage -p test_validation_runner.py -v
python3 tests/runtime_value_storage/check_flat_runtime.py --compiler gcc --compiler clang
```

The runner needs Python 3.10+ and CMake/CTest. It defaults to native Debug/Release
C builds, creates new `target/p07/<run>/` reports/logs/capability manifests, and
requires explicit outputs to be new/empty. Reject missing/zero/duplicate/disabled
tests and stale products. Timeouts terminate POSIX groups or Windows process trees.

`--full` additionally requires Rust formatting/checking, structured emitter tests,
ABI/cache checks, both active emitted-C smoke/lifecycle tests, public HDL in both
optimizer modes and the all-feature repository suite. Missing native waveform/
scheduler/coroutine coverage blocks full-host acceptance; generated HDL is native,
not implicitly sanitized. Exit 0 means requested checks passed, 1 failed, 2 blocked.

`--without-waveforms`/`--without-scheduler` explicitly select component-only coverage.
Windows MSVC CI configures values/containers only; macOS records actual architecture-
dependent coverage. Neither configuration nor Linux results certify those platforms.
The flat checker verifies fragment order and strict facade C11 compilation, accepts
GCC/Clang and cl/clang-cl with optional `--without-scheduler`, requires the current
ABI to compile and the stale ABI to fail. Linux execution does not validate MSVC.
