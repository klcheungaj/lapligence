# Embedded C11 simulation runtime

Generated models compile this runtime separately from Rust. Exact-width values use
unique ownership (legacy ABI 4, experimental compact ABI 5); registered scopes and retained destinations support
suspension, cancellation and queued publication. See
[value ownership](value/ownership.md) for the allocation contract.
Contiguous packed copies use masked limb transfers across value/X/Z planes;
selection clipping and alias snapshots preserve partial-range and overlap behavior.
Reversed packed ranges retain bit-wise traversal.
Proven non-escaping locals register caller-owned scope nodes and cells in
coroutine frames or frame-less functions' C scopes. Escaping and unproven cells
keep heap owners. [Retainer and lifetime rules](AGENTS.md#frame-resident-cells)
cover exact-pointer indexing, final history removal and zero-resume exits.

Fixed integral variable arrays above the dense threshold use
`scheduler/fixed_arrays.c`: one owned default, indexed stable exceptions and a
contents dependency. Reads borrow defaults without allocating cells. Whole copies
capture only exceptions plus the default; an NBA owns one descriptor snapshot.
Selected observers materialize stable cells. Model arrays are destroyed after
scheduler cleanup; lexical descriptors are registered objects drained on scope
exit or cancellation. All payload operations use the public value facade.

## Components

| Source pair / directory | Responsibility |
| --- | --- |
| `llg_value.h/.c`, `value/` | Four-state values, arithmetic, selection, resolution, formatting and numeric conversion; scheduler-independent. |
| `llg_random.h/.c` | Verilog random/distribution functions and explicit seed updates. |
| `llg_rng.h/.c` | Process/object random streams, independent of scheduling. |
| `llg_co.h/.c` | Stackless coroutine frames, anchors and arena cold paths; the generated-process contract is [process ABI version 3](process_abi.md). |
| `llg_string.h/.c` | Owned byte strings, conversion and change notification. |
| `llg_container.h/.c`, `container/` | Dynamic arrays, queues, associative storage and retained element identities. |
| `llg_rt.h/.c`, `scheduler/` | IEEE region scheduling, processes, events, assignments, synchronization, assertions, I/O and VPI. |
| `llg_wave.h/.c` | Optional asynchronous VCD/FST output with one producer/writer and flush/close barriers. |
| `gtkwave/` | Pinned libfst, compression support and provenance; included only for waveform models. |
| `vendor/zlib` (submodule) | zlib v1.3.2 sources libfst needs, embedded by `waveform_sources()` and written under `zlib/` only for waveform models. |
| `llg_rt_selftest.c`, `llg_wave_selftest.c` | Value/scheduler checks and VCD/FST lifecycle/reader checks. |

The scheduler keeps typed region queues, owned pending values and process origins.
Current-slot nonblocking assignments use an issue-ordered FIFO per region and a
separate per-process cancellation link. Future assignments live in time-ordered
buckets and are independent of the issuing process until their slot is promoted.
Selected nonblocking assignments retain an offset and narrow four-state slice;
the full mask representation remains only for non-contiguous fallback writes.
Resolved nets keep one generated interval-tree node per driver plus an exact-size
query scratch array. Immediate selected-driver updates resolve only the union of
their old and new intervals using overlapping drivers; publication still targets
the canonical full net, so waiter semantics do not depend on the optimization.
The process record embeds only time, named-event and expression-list wait
payloads. Rare wait kinds own one zeroed out-of-line payload that wake,
cancellation and teardown release through the same cleanup path.
Signal, real and process-await waiters subscribe to an address index. Each
source owns a reverse-registration list, matching the former global wait-list
order; waking still appends to the process region FIFO. Duplicate addresses
create one membership while the original descriptors and snapshots remain
intact. A stack-owned intrusive cursor makes removal of current or subsequent
members safe during publication, including nested publication. Empty source
rows are reclaimed immediately; the hash table retains only its peak capacity.
Global wait membership, handle membership and both activation memberships have
back-links for constant-time removal. Named-event table order remains governed
by its existing registration/swap-removal contract.

The process registry keeps stable slots and reuses the lowest vacant slot so
cancellation traversal order remains unchanged. A radix-64 free-slot bitmap
visits at most six levels for the entire int-indexed registry. Table growth and
trailing-hole trimming are amortized constant time per registration/removal;
ordinary slot removal uses the process's stored position. Cancellation traversals
that can remove other records restart as before.

On 64-bit hosts the process record is 424 bytes, including a 128-byte wait
record. Heap-path dependency memberships add 32 bytes each; a single narrow
signal wait embeds its membership. Each live source row adds 32 bytes, and each
hash bucket 8 bytes (geometric capacity at 75% load). Temporary key sorting uses
pointer integer representations, while
identity comparisons use exact pointer equality. Handles are 40 bytes (8 added),
activations 72 bytes (16 added); the process slot index occupies prior padding.
The free-slot bitmap uses about 0.127 bytes per allocated registry slot plus
at most five rounded summary words. These internal layouts keep process ABI 3;
runtime-content hashing invalidates older cached archives.

`LLG_WAIT_INLINE_SPECS` is one and `LLG_WAIT_INLINE_LIMBS` is one (64 bits per
word). Single packed any/edge waits and level targets through 64 bits copy their
spec and neutral A/B words into the wait payload, alongside one subscription.
Timed queue links share that union, keeping waits at 128 bytes and processes at
424 bytes. A single named-event wait copies its resolved object into an inline
list slot. Wide, multiple, mixed, expression and dependency waits keep owned heap
storage. Source rows and named-event tables still allocate on first use/growth;
there is no per-wait allocation for the inline paths after that shared storage is
available. Inline snapshots are copied words used only in synchronous
comparisons; no packed descriptor is fabricated over their storage. Native
signal-width growth promotes a live edge wait to heap storage while preserving
its subscription's exact list position.
Wake, cancellation and teardown unlink subscriptions before clearing the payload.

Stop/resume retains a live context; close releases observers/queues before model
storage. Runtime ticks are integer design-precision units; lowering supplies
per-module scaling. Memory text loading and binary reads keep their distinct
address-order rules.

## Embedding and build

`mod.rs` returns header/flat-source pairs through `value_sources`,
`random_sources`, `rng_sources`, `coroutine_sources`, `runtime_sources`,
`string_sources` and `container_sources`; waveform and self-test accessors provide
their related files. `value_sources_for` and `value_backend_sources` carry the
selected facade, nested headers and compact units. Facades include ordered private fragments; emitted sources
flatten the same order. `llg_co.c` is an independent translation unit, not a
scheduler fragment.

`llg_co` owns no global or thread-local state. The runtime supplies its allocation
hooks and owns one simulation-thread chunk cache, capped by
`LLG_CO_CHUNK_CACHE_MAX_BYTES` (1 MiB by default). Exact-size chunks are reused;
the exported counter snapshot records system allocations, cache hits, system
frees, current cached bytes and the peak. Runtime cleanup frees every retained
chunk. Under the future MT-1 design, each worker will own an equivalent cache.

`generate_model_sources` writes a self-contained CMake tree with `llg_co`. ABI 3
models define `LLG_MODEL_PROCESS_ABI` and initialize through
`llg_rt_init_with_args_and_precision`; there is no process-stack sizing input.
Debug initialization and coroutine symbols have separate `_debug` link names,
so debug/release model/runtime mixing fails before execution. Arena acquisition
clears stale debug liveness tags left by cancelled cached activations; embedded
frame reuse still requires the previous activation to have ended. Debug builds
fill fresh callee payloads and dead sibling overlays with `0xA5`, after releasing
lexical owners and before new initialization. Completed frames are poisoned after
common-return cleanup, covering early returns. Live parent fields and retained
heap cells are preserved. Release builds omit these fills.
Waveforms additionally need CMake Threads support. They compile the bundled
zlib with `Z_PREFIX` (and `Z_HAVE_UNISTD_H` outside Windows), so no system zlib
is found or linked and user DPI libraries may link their own.

The generated 256-call recursion guard remains the plain-function bound. On
POSIX, startup warns when `RLIMIT_STACK` is below the named 8 MiB estimate for
the scheduler, one polled segment and that guard. Generated MSVC projects reserve
the same default with `/STACK`.

## Validation and limits

[Native runtime tests](../../../tests/runtime_value_storage/readme.md) cover
allocation accounting, failure cleanup, values, queues, callbacks and waveform
transfers. [Repository tests](../../../tests/readme.md#dynamic-ownership-validation)
separately exercise real emission and public HDL. Component-only results, handwritten
output-shape probes and configured platforms do not establish full model acceptance.
Stackless coroutine probes run under ASan/UBSan without stack-switch exceptions.
Windows x64/ARM64 generated-model build/run lanes are configured in CI but remain
unvalidated until native results are recorded. Complete platform qualification
remains a separate gate.
Use [feature status](../../../docs/sim_features.md), not this component map, for
supported language contexts and outstanding qualification.

`value/udp.c` evaluates typed combinational primitive row masks independently of
the scheduler. `sv4_udp_eval` borrows inputs and table bytes for the call and
returns an owned one-bit result, preserving Z-as-X, source-order matching and
unmatched X. Value ABI 4 stays unchanged; source hashing changes the runtime cache.

Fork groups append and unlink in constant time through a parent tail and group
predecessor. A pending cursor visits newly created groups once at each blocking
boundary, preserving source order. Child lists append through a tail, and each
child process points to its owning list node for cancellation. On 64-bit hosts
these links add 24 bytes per process and 16 bytes per group (72 → 88 bytes).
`wait fork` completion tests only the group's parent rather than all waiters.
Named-event registrations keep table positions and repair the moved registration
on swap-with-last removal; trigger snapshots invalidate positions before waking.
This preserves the existing table/wake order. Removal work depends on the moved
waiter's event-list arity, not the event's waiter population. Single-event slots
fit the inline payload; other lists add four bytes per reserved event entry,
sharing the existing allocation. The 128-byte wait and 240-byte resume block
remain unchanged. These private layouts leave process ABI 3 and llg_co ABI 1
unchanged; runtime-content hashing rebuilds cached archives.

The experimental [compact value backend](value_gmp/readme.md) uses ABI 5 and
provides inline small values, canonical optional B storage and optional GMP wide
mul/div/mod/pow kernels. Generated sources and runtime archives can select it with
`LLG_VALUE_BACKEND=compact` and `LLG_COMPACT_KERNELS=portable|gmp`; GMP requires
`GMP_ROOT`. Selected compact builds include S1–S9 and native V06 consumer helpers.
