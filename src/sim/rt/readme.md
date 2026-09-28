# Embedded C11 simulation runtime

Generated models compile this runtime separately from Rust. Exact-width values use
unique ownership (ABI 4); registered scopes and retained destinations support
suspension, cancellation and queued publication. See
[value ownership](value/ownership.md) for the allocation contract.

## Components

| Source pair / directory | Responsibility |
| --- | --- |
| `llg_value.h/.c`, `value/` | Four-state values, arithmetic, selection, resolution, formatting and numeric conversion; scheduler-independent. |
| `llg_random.h/.c` | Verilog random/distribution functions and explicit seed updates. |
| `llg_rng.h/.c` | Process/object random streams, independent of scheduling. |
| `llg_string.h/.c` | Owned byte strings, conversion and change notification. |
| `llg_container.h/.c`, `container/` | Dynamic arrays, queues, associative storage and retained element identities. |
| `llg_rt.h/.c`, `scheduler/` | IEEE region scheduling, processes, events, assignments, synchronization, assertions, I/O and VPI. |
| `llg_wave.h/.c` | Optional asynchronous VCD/FST output with one producer/writer and flush/close barriers. |
| `gtkwave/` | Pinned libfst, compression support and provenance; included only for waveform models. |
| `llg_rt_selftest.c`, `llg_wave_selftest.c` | Value/scheduler checks and VCD/FST lifecycle/reader checks. |

The scheduler keeps typed region queues, owned pending values and process origins.
Current-slot nonblocking assignments use an issue-ordered FIFO per region and a
separate per-process cancellation link. Future assignments live in time-ordered
buckets and are independent of the issuing process until their slot is promoted.
Selected nonblocking assignments retain an offset and narrow four-state slice;
the full mask representation remains only for non-contiguous fallback writes.
Stop/resume retains a live context; close releases observers/queues before model
storage. Runtime ticks are integer design-precision units; lowering supplies
per-module scaling. Memory text loading and binary reads keep their distinct
address-order rules.

## Embedding and build

`mod.rs` returns header/flat-source pairs through `value_sources`,
`random_sources`, `rng_sources`, `runtime_sources`, `string_sources` and
`container_sources`; libaco, waveform and self-test accessors provide their related
files. Facades include ordered private fragments; emitted sources flatten the same
order. Only facades are compiled.

`generate_model_sources` writes a self-contained CMake tree, including libaco and
`aco_assert_override.h`. Normal builds may reuse a compatible runtime archive.
Model-specific stack headroom is passed at startup, not encoded in the archive ABI.
Waveforms additionally need zlib and CMake Threads support.

## Validation and limits

[Native runtime tests](../../../tests/runtime_value_storage/readme.md) cover
allocation accounting, failure cleanup, values, queues, callbacks and waveform
transfers. [Repository tests](../../../tests/readme.md#dynamic-ownership-validation)
separately exercise real emission and public HDL. Component-only results, handwritten
output-shape probes and configured platforms do not establish full model acceptance.
Real coroutine sanitizer and native Windows/libaco limits remain separate gates.
Use [feature status](../../../docs/sim_features.md), not this component map, for
supported language contexts and outstanding qualification.
