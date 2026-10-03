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

Full coverage needs a C11 compiler, CMake, threads and zlib. Scheduler/VPI/process
probes use portable `llg_co` frames on every C11 host.
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

The component sanitizer lane includes process suspension, cancellation, anchored
and arena calls because `llg_co` has no alternate native stack. Counts depend on enabled
capabilities; check the exact inventory rather than assuming a fixed count.

## Coverage

| Probe / group | Contract |
| --- | --- |
| `consumer_bridge_probe.c` | Allocation-free masked/range compare/copy/fill at unaligned offsets and 0/31/32/63/64/65/129/257-bit boundaries; foreign 32-bit record strides, partial imports, copied A/B snapshots, zero-extended text and modular digit parsing. |
| `neutral_access_probe.c` | Neutral shape/state/word/range mutation at 0..1,048,575 bits; literal Rust/DPI/VPI encodings, source-sign cast versus requested-sign resize, two-state coercion and independent/self-alias owner operations. The standalone compact probes exercise its V01 bridge independently. |
| `storage_probe.c` | Exact contiguous planes, masking, independent clone/copy/move, repeated destruction, 10,000 replacements, zero/exclusive widths and failure-atomic OOM. Fatal cases require specific diagnostics. |
| `stream_preflight_probe.c` | INT64 endpoints, declared bounds/traversal, unknown selectors, source-size rejection including later short segments, and zero remaining packed owners. |
| `array_conditional_probe.c` | Immediate-element equality/defaults versus packed mux, mixed X/zero defaults, aliased inputs, boundary/max widths, independent results, 10,000 replacements and malformed shapes. |
| `four_state_probe.c`, `value_ownership_probe.c` | Independent scalar state/sign tables, casez/casex, mixed widths through 4,097 bits, equality/mux/arithmetic/selections, widths 0..257, maximum values, conversion, nonmutation and steady ownership. |
| `udp_probe.c` | All five input masks, Z-as-X, first matching row, unmatched X, independent results and 10,000 ownership cycles. |
| `container_ownership_probe.c` | Recursive copies, alias-safe replacement, queue shifts/pops/pinned detached refs, normalized associative keys/defaults and teardown. |
| `port_net_collapse_probe.c` | All seven runtime kinds at 1/65/129 bits, strengths/defaults/release, delayed alias/array publication and zero owners; no frontend policy or stack switching. |
| `scheduler_ownership_probe.c` | Captured/masked NBA, scopes/frames, inertial/force state, sequence locals/endpoints, sampling, mailbox cleanup, wide formatting, plusargs and exact time scaling. |
| `coroutine_ownership_probe.c` | 1,000 actual yields and completion/cancel unwind, repeated 50 times. |
| `coroutine_chunk_cache_probe.c` | Stackless arena chunk reuse, byte-cap enforcement, cumulative counters and teardown release. |
| `coroutine_debug_probe.c` | Debug root/fork payload poisoning, root/static/anchor liveness failures (including the root while an anchor is active), invalid/poisoned dispatch state, finished/reentrant chain aborts and an exiting-process service call; a cancelled arena chunk is reused by a valid new activation. Failure cases require nonzero exit and the production diagnostic. |
| `coroutine_mode_model.c` | Both debug/release model/runtime link mismatches, separately through startup and arena symbols; successful matched-mode execution is covered by the scale probes. |
| `fork_group_scale_probe.c` | Indexed ordinary/triggered event swap removal and duplicate registrations; reverse removal, FIFO starts, wait fork, disable fork, parked teardown and one blocking boundary per new group at 100,000/200,000 children in release/debug modes; CPU-time scaling below 3.6× for 2× population and clean teardown. |
| `coroutine_scale_probe.c` | 8 MiB transient release, 32 simultaneous dynamic callees, eight suspend/return/park rounds in release and debug modes: 224 warmed calls reuse chunks with no new system allocations/frees, each parked chain has no arena chunks, and cleanup releases the shared cache. |
| `vpi_ownership_probe.c` | 129-bit X/Z puts/gets, 65,537-bit text scratch, result replacement, call cleanup and ten reinitializations. |
| `waveform_snapshot_probe.c` | Ring wrap/move clearing, mutation-after-capture, VCD/FST values, wide views, ignored/error events and pending close/reinit. |
| `wait_inline_probe.c` | Zero per-wait allocations after source/table warmup, 64/65-bit fallback, copied specs/nested dependencies, X/Z edges and levels, live snapshot widening, re-arm across timed/signal/event/heap paths, cancellation and teardown. |
| `scheduler_lists_probe.c` | 20,000 process slots, signal subscribers, dependencies and activations; lowest-hole reuse, reverse wake order, duplicate dependencies, current/next removal during nested publication, process awaits, cancellation and retained-handle teardown. |
| `clocking_index_probe.c` | Exact clock descriptor lookup, growth/tombstones/removal, freed/reused scopes, retained NBA lifetime, slot reset, persistent sequence ticks, four-state and multiple edges, teardown. |
| `scope_index_probe.c`, `callback_finish_probe.c` | Growth/tombstones/out-of-order releases, retained cells, finish during evaluators, shared per-field contexts and force/qualifier cleanup. |
| `frame_cells_probe.c` | Caller-owned packed/native registration, exact-pointer lookup/removal, nested mixed heap/frame scopes, retained heap NBA after lexical exit, cancellation, repeated reuse of a completed callee frame, zero-resume exit followed by runtime unwind, and fatal rejection of an escaped intrusive cell. |
| `event_array_probe.c` | Mixed declared directions, negative/out-of-range/X selectors and inert invalid waits. |
| `nextest_control_probe.c` | Native cancellation, staged outputs, lexical activation exits, inertial/strobe/force callbacks and repeated starts. |
| `generated_scopes_probe.c`, `generated_coroutine_probe.c` | Handwritten output shapes: lexical cells, retained NBA/clocking transfers, masks, recursion, yielding calls, finish/cancel and stop/resume/close. |
| `stackless_runtime_probe.c` | Deep polled/anchored/arena cancellation, CALL/READY scheduling boundaries, exact/deferred stop, final finish, budget exit kinds, semaphore/mailbox cancellation, and T28 arena/cache counters. |
| `native_ownership_probe.c` | Root string cleanup and independent copies; 15 input/callback modes repeated eight times, including scans/plusargs/containers and writer cancellation through stackless frames. |
| `native_boundaries_probe.c` | Ref scope/relocation/removal; nine mailbox/stream modes repeated eight times with reentrant delivery, peek, cancellation and returning termination. |
| `review_lifetimes_probe.c` | Native indexing, detached refs/tombstones/zero payloads and wide indices; stable real/shortreal mailbox targets and publication cancellation. Defensive API effects are not legal read-only HDL callback claims. |
| `word_copy_probe.c` | Independent per-bit reference for concat/repeat, shifts, stream/unstream, part/indexed selections and selection-plan read/slice/set at 1/7/63/64/65/127/128/129/1023/4096 bits; unaligned offsets, every limb boundary, X/Z, reversed/clipped ranges, aliased writes, raw-mask normalization and zero owners. |
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

Debug poisoning fills fresh root and embedded/arena callee payloads and dead
sibling overlays with `LLG_CO_POISON_BYTE` (`0xA5`); it does not itself trap an
arbitrary uninitialized payload read. `coroutine_poison_probe.c` reads those
bytes and checks that release macros preserve seeded storage and callee headers.
Generated overlay exits drain owners before poisoning; flattened parent storage
and retained heap owners remain live. Header liveness and dispatch checks catch the
explicit corruptions above. The standalone anchor-loop bad-state handler prints
the function/state; the scheduler's handler additionally prints an HDL backtrace.
Chunk counters count system allocations and cache hits, not value/wait allocations
or arena bump operations. Empty chain arenas can coexist with retained chunks in
the globally bounded cache. The scale probe deliberately fits that cache; a larger
simultaneously live dynamic-call working set can exceed it and allocate again.

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
Windows MSVC CI builds and runs a generated model, while macOS records actual
architecture-dependent coverage. Those configured lanes still require native-host
validation before they certify their platforms; Linux results do not substitute for it.
The flat checker verifies fragment order and strict facade C11 compilation, accepts
GCC/Clang and cl/clang-cl with optional `--without-scheduler`, requires the current
ABI to compile and the stale ABI to fail. Linux execution does not validate MSVC.

## Standalone compact value backend

The optional compact probes build `src/sim/rt/value_gmp/` separately from model
embedding. They link the live legacy backend for differential comparison and use
independent exhaustive <=4-bit state tables and Python integer vectors. Checks
include canonical B removal, exact payload bytes, ownership/aliasing, max width,
exclusive-limit rejection and allocation counts. Public headers remain GMP-free.

```sh
cmake -S tests/runtime_value_storage -B /build/llg-compact-gcc \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_C_COMPILER=gcc \
  -DLLG_STORAGE_TEST_WAVEFORMS=OFF -DLLG_STORAGE_TEST_COMPACT=ON \
  -DLLG_GMP_ROOT=/path/to/gmp-install
cmake --build /build/llg-compact-gcc --parallel 6 --target compact_checks
ctest --test-dir /build/llg-compact-gcc -R '^compact_' --output-on-failure --parallel 6
/build/llg-compact-gcc/compact_gmp_benchmark
```

The `compact_checks` target builds every enabled compact probe (S1–S3 and
S6–S9); S4/S5 remain pending. Omit `LLG_GMP_ROOT` for a dependency-free
portable build. Use a separate build directory with
`-DCMAKE_C_COMPILER=clang` for Clang, or add
`-DLLG_STORAGE_TEST_SANITIZERS=ON` for GCC ASan/UBSan. Allocation counter targets
use linker wrapping on ELF Unix hosts. Checks remain active under `NDEBUG`.
The Python harness communicates through standalone executables so a non-PIC
static GMP archive is sufficient. Each oracle executable returns copied logical
words, shape/sign and owned payload bytes, never a backend descriptor.

Benchmarks emit seven-sample medians and ranges for fresh results and initialized
destination reuse, including X/Z operands and 65-bit multiplication. Copy reuse
uses `sv4_copy`; arithmetic reuse uses the emitter's replace-of-fresh-result
pattern. These are indicative microbenchmarks, with no whole-model claim.

The S2/S3 family probes are enabled by the same compact option. Their separate
target is available for focused runs:

```sh
cmake --build /build/llg-compact-gcc --parallel 6 --target compact_v05a_checks
ctest --test-dir /build/llg-compact-gcc -R '^compact_.*families_' \
  --output-on-failure --parallel 6
/build/llg-compact-gcc/compact_portable_families_benchmark
```

The family harness exhausts four-state binary inputs of every width through four,
including unequal widths and all signs, and every same-width range triple through
four with all eight sign combinations. Wide checks cover the G1 width list and
maximum admitted width, count limb boundaries, high/unknown count words,
result independence, destination-as-operand replacement and canonical payloads.
Python supplies separate integer/state vectors for every family operation.
The counter probe checks allocation-free small operations and wide predicates,
independent zero-count shifts and B removal when all unknown bits shift out.
Family benchmarks emit seven-sample fresh-result medians/ranges for all 20 call
variants, including both onehot modes, known and X/Z at 1..4096 bits. Their
comparison operands are independent equal values so case scans execute fully.

The S6–S9 adapter probes add exhaustive two-driver strength endpoints, all UDP
mask/state combinations, enum order/defaults, full/unaligned-range resolution,
real/time conversion and failure cases, bounded radix/decimal formatting, exact
host indices, checked widths and source-compatible macro/reference types.
`compact_adapters_oracle.py` checks numeric/text results against Python integers,
rational rounding and struct bit conversions in both backends. Large finite
packed-to-real results permit at most one double ULP; exact bit reinterpretation
and real-to-integer conversion have exact expectations.

Build the additional targets in either configured compact build directory:

```sh
cmake --build /build/llg-compact-gcc --parallel 6 --target \
  compact_portable_net_adapters compact_gmp_net_adapters \
  compact_portable_real_time compact_gmp_real_time \
  compact_portable_format_index compact_gmp_format_index \
  compact_portable_facade_adapters compact_gmp_facade_adapters \
  compact_legacy_facade_adapters compact_adapters_oracle_legacy \
  compact_adapters_oracle_portable compact_adapters_oracle_gmp \
  compact_portable_adapters_benchmark compact_gmp_adapters_benchmark
ctest --test-dir /build/llg-compact-gcc -R '^compact_' --output-on-failure --parallel 6
/build/llg-compact-gcc/compact_gmp_adapters_benchmark
```

The adapter benchmark measures seven alternating-order samples at 1/64/65/256/
4096 bits, known and X/Z, including fresh owned results and scalar/text inspection.
Bit reinterpretation and rtoi have their fixed 64/32-bit result widths; real delay
is a native scalar. Packed delay uses a representable low-limb input.
Checked-width timing uses valid
known inputs; its X/Z rejection is exercised by the failure probes. No model
selection, scheduler adoption or native non-Linux qualification is implied.
