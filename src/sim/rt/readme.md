# Embedded C11 simulation runtime

Generated models compile this runtime separately from Rust. Exact-width values use
unique ownership (compact ABI 5; the legacy development reference uses ABI 4); registered scopes and retained destinations support
suspension, cancellation and queued publication. See
[value ownership](value/ownership.md) for the allocation contract.
Contiguous packed copies use masked limb transfers across value/X/Z planes;
selection clipping and alias snapshots preserve partial-range and overlap behavior.
Reversed packed ranges retain bit-wise traversal.
Every value-, string- or runtime-returning operation that generated code calls
also has a destination form `X_to(dst, ...)` (`value/destinations.h`, and blocks
at the end of `llg_rt.h`, `llg_container.h`, `llg_string.h`, `llg_vpi.h`) taking
packed operands by address, so generated frames hold no per-call-site
returned-descriptor temporaries; the returning forms remain for runtime code.
Proven non-escaping locals register caller-owned scope nodes and cells in
coroutine frames or frame-less functions' C scopes. Escaping and unproven cells
keep heap owners. [Retainer and lifetime rules](scheduler/readme.md#frame-resident-cells)
cover exact-pointer indexing, final history removal and zero-resume exits.

Fixed integral variable arrays above the dense threshold use
`scheduler/fixed_arrays.c`: one owned default, indexed stable exceptions and a
contents dependency. Reads borrow defaults without allocating cells. Whole copies
capture only exceptions plus the default; an NBA owns one descriptor snapshot.
Selected observers materialize stable cells. Model arrays are destroyed after
scheduler cleanup; lexical descriptors are registered objects drained on scope
exit or cancellation. All payload operations use the public value facade.

Typed display formatting (`scheduler/formatting.c`) formats ordinary packed
scalars in frame scratch without heap allocation. `%p` text comes from
`scheduler/patterns.c`, which walks a value read-only through the model's
`llg_pattern_types` table and registered class layouts: cycles print
`(cycle)`, objects deeper than `LLG_PATTERN_MAX_DEPTH` print `(...)`,
reclaimed objects print `(reclaimed)` and output stops at
`LLG_PATTERN_OUTPUT_LIMIT` with a warning.

## Components

| Source pair / directory | Responsibility |
| --- | --- |
| `llg_value.h/.c`, `value/` | Four-state values, arithmetic, selection, resolution, formatting and numeric conversion; scheduler-independent. |
| `llg_random.h/.c` | Verilog random/distribution functions and explicit seed updates. |
| `llg_rng.h/.c` | Process/object random streams, independent of scheduling. |
| `llg_co.h/.c` | Stackless coroutine frames, anchors, arena cold paths and the synchronous driver for recursive subprograms; the generated-process contract is [process ABI version 3](process_abi.md). |
| `llg_string.h/.c` | Owned byte strings, conversion and change notification. |
| `llg_container.h/.c`, `container/` | Dynamic arrays, queues, associative storage and retained element identities; `container/bitstreams.c` holds runtime-sized bit streams (`llg_bitstream_t`) for streaming and bit-stream casts of strings, resizable containers and oversized arrays, built only through the public value facade. |
| `llg_rt.h/.c`, `scheduler/` | IEEE region scheduling, processes, events, assignments, synchronization, assertions, I/O and VPI. |
| `coverage/` | Embedded in `llg_rt.c` after the scheduler. `coverage.c`: the covergroup store (SIM-044, SIM-045): instances, dense and sparse counters, transition automata, crosses, options and the 19.8/19.11 queries. `database.c` (SIM-046): the versioned, checksummed, host-independent coverage database behind `$set_coverage_db_name`, `$load_coverage_db`, `$coverage_save` and `$coverage_merge`, with atomic loads and saves. `assertions.c` (SIM-046): assertion coverage items and their 40.5.3 counters, and the 40.3.2 `$coverage_*` functions. |
| `llg_vpi.h/.c`, `vpi_user.h`, `sv_vpi_user.h` | VPI bridge (SV clauses 36-38, Annex K/M constants): the generated object catalog (modules, interfaces, generate scopes, named blocks, ports, parameters, nets, variables, arrays, unpacked structures and their member rows, and the structure objects: named events, processes, continuous assignments, tasks and functions, primitives, instance arrays, modports, module paths and timing checks, with their expression connections in `llg_vpi_conn_t` tables) indexed lazily on first use; the generated code tables (`llg_vpi_code_tables`, registered by `llg_vpi_model_code`, NULL without a catalog) with the expression and statement trees under those objects, evaluated and decompiled on demand ([io](io.md)); the generated type tables (`llg_vpi_type_tables`, registered by `llg_vpi_model_types`, NULL without a catalog): interned typespecs with their ranges, typespec members, enum constants and, for each structure or union of fixed packed width, a member tree of absolute bit offsets that member handles (row, element, node) index without copying; typespecs, typespec members and enum constants are static handle slots after the catalog rows; generation-checked handles for catalog objects, array elements, structure and union members, bit/part-selects, ranges, iterators, constants, calls, arguments and scheduled events; every Table 38-3 value format with exact X/Z; `vpi_put_value` with no delay, inertial/transport/pure-transport delays (Active-region events of the target slot, carried by runtime carrier processes, decision S42-D5), event handles and cancellation, force and release; user system tasks and functions (compiletf, sizetf once per registration, calltf, `vpiArgument`, per-call-site user data, 38.34 return conventions); simulation callbacks (SIM-043): `cbValueChange` through runtime value watches, the time reasons in their Table 4-1 PLI regions, the compile/start/end actions, removal and registration while callbacks run; read-only puts rejected; `vpi_control` stop/finish; `vpi_get_delays`/`vpi_put_delays` on paths, checks, primitives and continuous assignments through `llg_sdf_apply` and `cbTchkViolation` (VPI-T1). Assertion handles (SIM-046, SV 39.3) from `vpi_iterate(vpiAssertion, ...)` and `vpi_handle_by_name`, with their type, names, scope and 40.5.3 counters, `vpi_get(vpiAssertCoverage, instance)` and the `vpi_control` coverage operations; `sv_vpi_user.h` holds the Annex M assertion and coverage constants. Dynamic objects (VPI-O4, SV 37.17, 37.27-37.29) live in `llg_vpi_dynamic.c`, embedded right after `llg_vpi.c` in the same translation unit: catalog rows flagged `LLG_VPI_OBJ_DYNAMIC` name string, chandle and class variable storage or a container with a shape from the generated `llg_vpi_dynamic_tables` (`llg_vpi_model_dynamic`; shapes, class layouts and the model's object and field accessors, NULL without a catalog); class objects, properties and container elements are handles that track (never pin) collected objects through a live table of `vpiObjId`s; `cbCreateObj` runs from the generated `llg_vpi_object_created` call after construction and shallow copy (catalog models only), `cbReclaimObj`/`cbEndOfObject` from the collector's reclaim hook (`llg_gc_reclaim_hook`), after which every handle into a reclaimed object is released. Runtime structures (VPI-F7: user systf, callback and time-queue lists, UDP definitions and table entries, `vpiUse`, threads, frames and their callbacks) live in `llg_vpi_runtime.c`, embedded after `llg_vpi_dynamic.c`; the time queue and threads read the scheduler through `scheduler/vpi_support.c` on demand. A model built without a catalog (`llg_vpi_objects` NULL) still runs applications, which find no objects. Applications load from `LLG_VPI_PLUGIN` or, linked as a static archive, through `llg_vpi_startup_static` (`LLG_VPI_STATIC_STARTUP`). `vpi_user.h` declares `vlog_startup_routines` with `PLI_DLLESPEC` as Annex K does, so a Windows DLL exports it. |
| `llg_dpi.h/.c` | DPI-C import bridge (SV Annex H), embedded after `llg_vpi.h/.c` so every model exports the `svdpi.h` routines to `--dpi-lib` libraries: canonical vector export/import, C-layout aggregate conversion driven by generated layout tables, the `svOpenArrayHandle` object, the H.11-H.12 access and query routines, `svDpiVersion` and the 35.9 imported-task protocol check. Import thunks with open-array formals run their foreign call under an `llg_dpi_guard_t` (`setjmp`); a fatal element access with the wrong index count (S40-D6) jumps back to the innermost guard, abandoning that C code. Foreign code only sees thunk-owned buffers valid for one call. It also keeps the DPI-C call-chain state (SIM-041): the model's sorted `llg_dpi_scope_t` table (`llg_dpi_model_init`), each import thunk's save/set/restore (`llg_dpi_import_enter/leave`), export entry resolution in the current scope (`llg_dpi_export_target`, `llg_dpi_export_enter/leave`), the H.9.3 scope, user-data and caller routines, `svIsDisabledState`/`svAckDisabledState` and the 35.5.3/35.9 fatal protocol checks. User data is freed by `llg_vpi_shutdown` through the hook `llg_dpi_model_init` registers (`llg_vpi_on_shutdown`), so `llg_vpi.c` with `llg_vpi_dynamic.c` builds without it (storage probes). |
| `foreign_contexts/llg_dpi_context.c` | Foreign call contexts (ADV-018, [design](foreign_contexts/readme.md)), written as `llg_dpi_context.c`, a runtime translation unit of its own that only models with suspendable DPI-C import shims link (with the platform thread library): pooled threads that run an import thunk's foreign call on their own stack under a run token, the hand-off of a waiting exported task's frame to the scheduler thread, and the unwinding (kill) or abandoning (end of simulation) of suspended chains through the process-end hook (`llg_rt_set_process_end_hook`). |
| `svdpi.h` | The IEEE 1800-2009 Annex I DPI header, written beside every generated model for the model and for `--dpi-lib` C code. It differs from the Annex I text only in defining the fixed-width types through `<inttypes.h>` on every host (I.2 leaves the method open; the suggested `<sys/types.h>` fallback lacks `uint8_t`/`uint32_t` on macOS) and in declaring `svScope` and `svOpenArrayHandle` without `XXTERN`, whose `__declspec(dllimport)` MSVC would report as ignored on a typedef. A Windows DLL marks its import definitions `DPI_DLLESPEC` (exported) and calls the routines through the model's import library (`llg --dpi-import-lib`). |
| `llg_wave.h/.c` | Optional asynchronous VCD/FST output with one producer/writer and flush/close barriers. |
| `llg_compiler.h`, `llg_platform.h`, `llg_platform_native.h` | The platform layer: every compiler and OS conditional (atomics, threads, dynamic libraries, host paths/directories, stack limit, console log process) behind neutral inline functions. `llg_compiler.h` is safe for generated models; the other two are private to runtime `.c` files. |
| `vendor/libfst` + `patches/libfst` | Pristine upstream libfst/FastLZ/LZ4 snapshot and its tracked local patch; the build script applies the patch in place in `vendor/libfst` (like Slang), and `waveform_sources()` embeds those files only for waveform models. The applied state is a build product and is never committed. |
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
A process's write to a net with several driver slots defers that resolution to
the end of its region queue drain, keeping the union of the written intervals
in one table entry per net (decision S2-D11).
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

On 64-bit hosts the process record is 440 bytes, including a 128-byte wait
record and the pointer to a mailbox delivery it has not resumed to take (the
message returns to its mailbox if the process is killed first). Heap-path dependency memberships add 32 bytes each; a single narrow
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
432 bytes. A single named-event wait copies its resolved object into an inline
list slot. Wide, multiple, mixed, expression and dependency waits keep owned heap
storage. Source rows and named-event tables still allocate on first use/growth;
there is no per-wait allocation for the inline paths after that shared storage is
available. Inline snapshots are copied words used only in synchronous
comparisons; no packed descriptor is fabricated over their storage. Native
signal-width growth promotes a live edge wait to heap storage while preserving
its subscription's exact list position.
Wake, cancellation and teardown unlink subscriptions before clearing the payload.
A dependency wait (`W_DEPS`) is the exception: its process keeps the copied
list, snapshots and memberships in one `llg_wait_retained_t` (the process
record's `retained_wait`, 8 bytes). A wake leaves the memberships linked with
a NULL waiter, which publication skips like a cursor; re-arming the same list
(every continuous assignment, gate, UDP and `@*` loop) refreshes the snapshots
and moves each membership to the head of its source list, the position a fresh
registration takes, so wake order is unchanged and no allocation, sort or
source lookup runs per wake. Arming a different list, completion, kill and
teardown release the record. The development-only C define
`LLG_DEV_LEGACY_GATE_PATH` restores per-wake registration, and the net and
scope paths below, for differential tests.

Net writes keep the driver interval index exact but cheap: a whole-slot write
indexes the driver by its driven (non-Z) extent rather than the full net, a
selected write over the same range keeps its node, and a same-width unchanged
driver returns before any resize. Publication snapshots and resolved net
values live in private value scopes (`value_scope_begin_private`): they are
never write, NBA or clocking targets, so they skip the address index and
clocking history and allocate node and cells together. Pending inertial
updates form a doubly linked list sorted by time (FIFO among equal times);
insertion searches from the tail and the due-now scans stop at the first later
time.

Stop/resume retains a live context; close releases observers/queues before model
storage. Runtime ticks are integer design-precision units; lowering supplies
per-module scaling. Memory text loading and binary reads keep their distinct
address-order rules.

## Random streams

`llg_random.c` is the Annex N algorithm for `$random` and `$dist_*`; the seed is
a caller-owned in/out 32-bit integer and the implicit `$random` seed starts at
0. `llg_rng.c` owns every other stream (`llg_rng_state_t`: PCG state,
odd stream increment, child count):

- A static process (spawned by the model) is seeded with the next value of its
  instance's initialization stream. `scheduler/process_registry.c` keeps one
  stream per instance in an open-addressing table keyed by the process label
  without its kind component (`tb.u.always` -> `tb.u`); each starts from the
  default seed (SV 18.14.1), so identical instances draw identical values. The
  model enables this with
  `llg_rt_use_instance_random_streams`, before its static initializers, when
  it observes any stream; other models seed static processes from the root
  stream in spawn order and skip the table. A static handle initializer runs
  in the initialization frame between `llg_rng_initializer_scope_begin(label)`
  and `_end()`: objects it creates draw from that instance's stream
  (`tb.u.initializer` -> `tb.u`, SV 18.14.1). A forked or detached child is
  seeded with its parent's next value.
- A class object keeps a stream only when the model inspects object streams
  (`IrRandomUse::objects`): `llg_object_rng_create` seeds it with the creating
  thread's next value; a model that only draws thread values consumes that
  value without keeping state (`IrRandomUse::threads`), and other models pay
  nothing. A shallow copy copies the state and takes no thread value.
- `shuffle` draws from the stream `llg_container_set_rng_source` names; the
  scheduler installs the current thread's stream.
- `llg_randcase_select` (`scheduler/process_registry.c`) serves `randcase`
  (SV 18.16) with one draw from the calling thread's stream: a total up to 32
  bits uses `llg_rng_state_uniform(sum - 1, 0)`, exactly the value
  `$urandom_range(sum - 1, 0)` returns; a total up to 64 bits combines two
  32-bit draws with rejection; a wider one runs on the `sv4_t` facade, drawing
  just enough 32-bit words for the bit length of the sum and rejecting values
  at or above it. Lowering passes the exact-sum width (widest weight plus
  `ceil(log2(items))` bits), so the path is chosen per site and the sum never
  wraps. All-zero weights draw nothing and warn; a negative signed weight is
  an error that ends the run without final procedures (`llg_rt_skip_finals`
  before a fatal report) and returns -1; the generated statement takes its
  exit check.
- `llg_randsequence_select` chooses a `randsequence` rule (SV 18.17.1) with the
  same draw as `llg_randcase_select`; a negative (by its own signedness)
  weight is a located error that ends the run the same way and returns -1
  without drawing; all-zero weights warn and return -1.
  `llg_randsequence_join_pick` takes one `rand join` step (SV 18.17.5,
  S16-D6): with p = |2r - 1| it restricts the candidates to the longest
  (r > 0.5) or shortest (r < 0.5) remaining sequences with probability p,
  drawing one 32-bit value only when 0 < p < 1, then draws uniformly among more
  than one candidate; it returns -1 when every sequence is exhausted.

`get_randstate` returns the versioned, host-independent text
`LLG_RNG_V1:<state>:<increment>:<child count>` (three 16-digit hexadecimal
fields, 61 bytes). V1 names the algorithm: PCG-XSH-RR 64/32 output, splitmix64
seeding (`llg_rng_state_seed`) and child seeding with the parent's next 32-bit
value. The string holds no host pointer, so a saved state reproduces the same
values in another build or process of the same version, which is what later
constraint solving or checkpointing can rely on. `set_randstate` accepts only a
well-formed V1 string with an odd increment and otherwise leaves the stream
unchanged and reports `invalid randstate string`. A future algorithm must use a
new version tag and either accept V1 strings or reject them the same way.

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

The generated 256-call guard remains the plain-function bound. On POSIX,
startup warns when `RLIMIT_STACK` is below the named 640 KiB estimate: 367 KiB
measured for the scheduler, one polled segment, a guard-deep function chain and
the runtime helpers, plus 256 KiB of headroom for user DPI/VPI C code and libc,
which share the stack and cannot be bounded by the runtime. Default 8 MiB
stacks, including the 8176 KiB macOS reports, stay quiet. Generated MSVC
projects reserve the larger of the estimate and the 1 MiB Windows default with
`/STACK`.

Only facade translation units compile; the fragment order above must match the
`include_str!`/`concat!` flat embedding. Never include value
implementation in the scheduler or export private shared state to dodge assembly
rules. Value/random/RNG/`llg_co` sources are independent; runtime, string and
container sources require values, and containers do not require the scheduler.

Shared-body startup uses `llg_spawn_instance_in_region` or its program variant
(contract in `llg_rt.h`): the copied record pointer is checked against the header
and descriptor frame size, no record ownership or cleanup transfers to the
runtime, and roots without an instance-record field keep the other spawn services.

## Platform notes

General platform handling: [cross-platform guide](../../../docs/cross_platform.md)
([Windows](../../../docs/cross_platform.md#windows-msvc),
[macOS](../../../docs/cross_platform.md#macos),
[arm64](../../../docs/cross_platform.md#arm64)).

- Platform layer: [platform layers](../../../docs/cross_platform.md#platform-layers)
  and the `llg_compiler.h`, `llg_platform.h`, `llg_platform_native.h` header
  comments; generated C and runtime probes use it too. Exceptions: `llg_co.h` (own `LLG_CO_*` macros) and bundled
  third-party sources, whose required changes are tracked patches in
  `patches/libfst` ([patch docs](../../../patches/README.md#libfst)); never
  commit the applied state.
- Generated models and runtime sources build warning-clean with
  `-Wall -Wno-unused-function` (MSVC `/W3`); flags and levels are in the
  [model build](../build/readme.md).
- Standalone probes also build with MSVC `/W4 /WX` (at `/O2` for the
  `strict_c11_executable` tests); the warnings to avoid are listed under
  [MSVC compiler](../../../docs/cross_platform.md#windows-msvc). The
  `runtime_strict_sources` suite (`tests/runtime_strict_sources.rs` docs) runs
  Clang approximations and a constant-cast scan over the runtime units, the
  probes and the C fixtures
  ([guards](../../../tests/readme.md#msvc-and-windows-guards-on-linux)). Generated models
  still forward-declare descriptors as tentative definitions, because mutual
  recursion needs it; they build at `/W3`.
- User DPI/VPI C code shares the host stack and is covered only by the stack
  headroom above; the C code of a suspendable context import runs on a
  foreign call context with its own `LLG_DPI_CONTEXT_STACK_BYTES` stack.

## Validation and limits

[Native runtime tests](../../../tests/runtime_value_storage/readme.md) cover
allocation accounting, failure cleanup, values, queues, callbacks and waveform
transfers. [Repository tests](../../../tests/readme.md#dynamic-ownership-validation)
separately exercise real emission and public HDL. Component-only results, handwritten
output-shape probes and configured platforms do not establish full model acceptance.
Stackless coroutine probes run under ASan/UBSan without stack-switch exceptions.
A Windows x64 generated-model build/run lane is configured in CI but remains
unvalidated until native results are recorded; Windows on ARM supports `llg_ls`
only, so it has no generated-model lane. Complete platform qualification
remains a separate gate.
Use [feature status](../../../docs/sim_features.md), not this component map, for
supported language contexts and outstanding qualification.

`value/udp.c` evaluates typed combinational primitive row masks independently of
the scheduler for definitions too wide for the emitter's dense index (more than
10 inputs). `sv4_udp_eval` borrows inputs and table bytes for the call and
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

The [compact value backend](value_gmp/readme.md) uses ABI 5 and provides
inline small values, canonical optional B storage and GMP wide mul/div/mod/pow
kernels. Every generated model and runtime archive uses it (development builds
can select the legacy reference or portable kernels with
`LLG_DEV_VALUE_BACKEND`/`LLG_DEV_COMPACT_KERNELS`); GMP uses the bundled subset in [`gmp.rs`](gmp.rs) (written under `gmp/` and built by
`gmp/llg_gmp.cmake`; tables from `scripts/gmp_tables.py`) unless `GMP_ROOT`
names an installation. Selected compact builds include S1–S9 and native V06 consumer helpers.
