# Embedded C11 runtime

Compile this runtime with generated models, never into Rust binaries. Read
[pipeline](../AGENTS.md), [lowering](../codegen/AGENTS.md),
[emitter](../emit_c/AGENTS.md) and [value ownership](value/ownership.md).
The [value facade contract](value/facade.md) owns backend selection, neutral
accessors, width/sign/encoding rules and the feature-agent migration boundary.
Feature/status inventories belong in [sim_features](../../../docs/sim_features.md).
Keep original runtime/waveform self-tests active; component tests do not replace
Rust-emitted model, HDL, parity, sanitizer or native-platform acceptance.

## Standalone compact backend

`value_gmp/backend.h` uses ABI 5 and supplies G1, the V01 bridge and
V05/S1–S9 selections, references, assembly and adapters, plus V06 consumer
primitives. See
`value_gmp/facade_audit.md` for the remaining surface.
Experimental `ValueConfig` selection embeds it, with legacy as the default.
Never add a legacy fallback for an unavailable compact operation. Build its
separate translation units through
`tests/runtime_value_storage/compact_value.cmake`. Plain C kernels are always
available; `LLG_SV4_GMP_KERNELS=1` accelerates wide mul/div/mod/pow and decimal
conversion. Include GMP only in `kernels.c`, require 64-bit nail-free limbs,
never alias `uint64_t` words as a distinct limb type, and keep all other
operations on direct word loops. The `compact_checks`
CMake target builds the complete standalone compact inventory. The header inlines <=64-bit operations
and supports static constants through `LLG_GMP_SV4_LITERAL`.

The wide B plane exists exactly when a logical bit is X/Z. Every mutation/result
must mask top padding and remove a zero B plane before publication. No retained
capacity or public compact/workspace API. Results own independently; by-value
operands borrow for the call. Preserve requested-sign resize versus source-sign
cast. Test against live legacy, Python integers and exhaustive small states,
with both kernel configurations; never import a legacy descriptor as a fallback.

## Platform layer

Compiler and operating-system conditionals (`_WIN32`, `_MSC_VER`, `__APPLE__`,
`__unix__`, `__GNUC__`, `__clang__`, feature-test macros) live only in
`llg_compiler.h`, `llg_platform.h` and `llg_platform_native.h`; every other
runtime source, the emitter's generated C and the runtime probes call their
neutral names. Exceptions: `llg_co.h` stays a self-contained two-file library
with its own `LLG_CO_*` macros, and bundled third-party sources (`vendor/libfst`,
`vendor/zlib`) keep upstream conditionals; required libfst changes are tracked
patches in `patches/libfst`, applied in place by the build script. Never commit
the applied state (see [patch docs](../../../patches/README.md#libfst)). Layer functions are `static inline`
so hot paths (waveform ring, atomics) pay nothing. Public headers and generated
models may include only `llg_compiler.h`; `llg_platform_native.h` brings in
`<windows.h>` and is limited to the waveform writer and VPI bridge. MSVC uses
Interlocked intrinsics, not `<stdatomic.h>` (which needs
`/experimental:c11atomics` and VS 2022 17.5+). Add a service to the layer
before adding a conditional anywhere else.

## Values and native owners

Generated models use the compact backend, ABI 5. The legacy development
reference's `LLG_VALUE_ABI_VERSION` is 4, independent of model width. The legacy `sv4_t` stores
`uint32_t width`, `int8_t is_signed` and bits/x/z pointers into one allocation of
three `uint64_t` limb planes, each ceil(width/64). Width zero allocates nothing;
packed widths stay below exclusive `LLG_SUPPORTED_WIDTH_LIMIT` (`1 << 20`). Initialize with `SV4_EMPTY`, use
clone/copy/move/replace/destroy, and free only bits. Never retain struct copies or
interior pointers across move/replace/destroy. Packed inputs are borrowed unless
documented otherwise; every value-returning operation owns an independent result.
Containing objects destroy fields; coroutine-live values use registered scopes
unwound on completion/cancellation. No cleanup attributes, VLAs, alloca or C++
destructors. Scope indexes use exact pointer equality, never ordering; retain
lexical cells only as long as queued/ref users require them.

Combinational UDP row scans (`sv4_udp_eval`, used only for definitions above the
emitter's dense-index limit) borrow validated immutable row masks and scalar input
pointers only for the synchronous call, returning an independent unsigned one-bit
owner. Normalize input Z to X, scan in source order and return X if no row
matches. Dense-index evaluation needs no runtime operation beyond the bridge's
`llg_sv4_state` and `sv4_fill`. The additive API leaves value ABI 4 unchanged;
`value_sources` embeds `value/udp.c` in facade order, so runtime-content cache
hashing invalidates earlier archives.

Keep `llg_value` scheduler/coroutine/waveform-independent, parity with `core::elab`,
X/Z distinction, actual-width arithmetic/conversions and partial-select X fill.
Array-conditional helpers borrow inputs and return independent owners with
immediate-element defaults. Strength resolution retains both X endpoints, strict
opposite-endpoint dominance, neutral Z and wired-AND/OR tie rules; pull/supply
biases are sources. A highz endpoint makes an X contribution one-sided (L/H).
`llg_net_t.strength` (scheduler `nets.c`) is an optional per-bit strength view
for `%v`: the hull of every level choice, published after the value (with a
pending net delay, at the delayed commit), forced bits at strong; nets without
it pay one NULL test per resolution. Query counts ignore X/Z except isunknown. Numeric/time
conversion rules belong to lowering/value APIs, not host int/long assumptions.
`llg_random` uses modulo-2^32 seed arithmetic and checked/clamped distribution
conversions following Annex N. Process/object RNG streams consume one parent draw
when creating a child, not when operating on an existing child.

Strings clone reads, consume expression operands and replace/update mutations.
Stable callback markers belong only to persistent storage; unchanged writes do
not notify and clones carry no callback. Exclude NUL, use ASCII case conversion,
parse decimal prefixes rather than C hex/NaN/infinity in atoreal, and preserve
finite-double round trips in realtoa. Register every owner spanning callbacks;
multi-event contexts adopt each field before any evaluation, even shared pointers.

Containers require paired init/destroy. Preserve resize prefixes/default growth,
queue bounds/discard rules, ordered integral/byte-string associative keys, and
invalid-index default-read/no-op-write semantics. Reject X/Z integral keys before
key casting. Allocation overflow/exhaustion is fatal. Descriptor-backed dynamic
array elements use type operations; do not apply packed-only queue/associative
paths to other types. Packed `sort`/`rsort` evaluate each `with` key once, merge-sort
an index permutation without recursion, then permute data and queue element
identities in place; X/Z-key elements stay fixed. Queue refs use LIFO
`llg_ref_scope_t`: retain descriptor/cell
through copy-out/cancellation; snapshot/disconnect removed identities before
structural mutation. Destruction cannot free pinned cells, and ref lists are
borrowed. Alias reads never publish or mutate resolved storage.

Identity-handle elements (SIM-006) copy the handle: events, opaque objects and
borrowed chandles are never duplicated; `LLG_VALUE_PROCESS` handles are counted
through `llg_value_handle_hooks_t` (retain on copy, release on drop, a pop moves
the reference out). The scheduler installs the hooks at init and never clears
them; `event_new` creates scheduler-owned events for new event elements (Table
6-7), freed in `llg_rt_cleanup`, while missing or invalid reads yield null
(Table 7-1). Element locators (`container/value_elements.c`) return borrowed
element pointers valid until the next structural change; callers write members
in place and then call the container's `*_touch`. Associative reads through an
invalid key or of a nonexistent entry warn (SV 7.8.6) unless an explicit default
is set; invalid-key writes warn and do nothing. Activation and class-property
containers run without dependencies or notification.

Recursive equality (SIM-007) `llg_value_equality` returns unequal, equal or
unknown: a known packed mismatch dominates, strings compare bytes, handles
identities and reals numerically; `llg_{dyn,queue}[_value]_equal` compare
whole containers element-wise. `llg_dyn_value_copy_range` and
`llg_dyn_value_merge` serve fixed-array views (slices, ambiguous
conditionals). A descriptor-array NBA (`llg_dyn_value_nba`, `is_values`)
owns a deep copy of its payload from issue to commit or cancellation.
`llg_value_item_{to,from}_{dyn,queue,dyn_value,queue_value}` move a record's
queue or dynamic-array member between its nested slot in a value (null when
empty) and a standalone container; they never notify the slot's owner.

## Scheduler, process and event invariants

### Frame-resident cells

Process ABI 3 exposes the scope-node layout and adds caller-owned packed/native
registration. Value ABI 4 and the independent llg_co ABI 1 are unchanged. Scope
links are runtime-owned even when node/cell memory belongs to a frame or outer
C scope. Do not copy a live node or register it twice. Register only empty
packed/zero-initialized native cells, and reinitialize after end before reuse.
Normal end, disable, kill and cleanup use the existing lexical scope chain and
destructor order; drain it before destroying root/embedded/arena frames. Retained
heap cells still detach until final release. Intrusive scope end requires exactly
one lexical reference; an outstanding retain fails before unlinking the owner.
Both paths keep the exact-pointer index and call `llg_clocking_forget_signal`
on every packed descriptor at final release.
Coroutine nodes/cells must reside in the frame even without resume sites:
termination can return from the C entry before the runtime drains its owners.
Only frame-less functions with common-return cleanup may use C-stack cells.

The emitter's [typed proof](../emit_c/AGENTS.md#storage-references-and-publication)
excludes the following retainers or supplies the indicated cleanup:

| Descriptor retainer | Lifetime rule for intrusive locals |
| --- | --- |
| `nonblocking.c` NBA/selected/masked/future/event-controlled writes and `dependencies.c` pending clocking drives | NBA target locals remain heap-backed; clocking/event captures are unproven. |
| `activations.c`, `reference_writes.c`, queue refs, output/inout/ref and foreign call descriptors | All address actuals remain heap-backed; reference descriptors themselves retain their existing heap scopes. |
| `storage.c` capture slots and `forks.c` joined/detached descendants | Captured forks and spawning callee effects fail the proof. |
| Wait snapshots, R2 subscriptions, expression/iff contexts and `wait_order` delivery | Local wait dependencies and context-free evaluator reads remain heap-backed; captured evaluator/qualifier contexts are unproven. Existing wake/cancel paths remove subscriptions before scope unwind. |
| R1 `dependencies.c` clock history | Every final scope release forgets the exact packed descriptor before reuse; publication without a waiter is still recorded. |
| Clocking sources, sampled histories, concurrent assertions, sequence clocks/local scopes | Clocking/sampled/assertion forms fail the proof; external owners retain their documented lifetime obligations. |
| `nets.c` inertial handles/targets, `force.c` targets/evaluators, PCA bindings | These registrations fail the proof; model-lifetime sources keep their existing path. |
| `mailboxes.c` delayed delivery, pinned targets and reentrant publication snapshots | Mailbox/native service operations fail the proof. |
| VPI catalog/callback/call handles, DPI/foreign pointers and dynamic dispatch | These operations fail the proof; no frame cell is exposed through them. |
| Monitor/strobe/output callbacks and deferred assertion reports/action frames | Deferred readers/capture graphs fail the proof; legal persistent sources remain model-owned. |
| Synchronous write `value_target_pin` | The pin ends before the write returns. Proven locals have no waiter/foreign callback observer that could retain the target beyond scope end. |
| Container/object/alias/net and waveform registrations | These forms fail the proof. Ordinary by-value packed/string reads clone payloads; waveform events and formatting buffers own independent snapshots. |

Unknown typed operations fail closed. Native callers must separately meet the
same retainer obligations; registration never turns a borrowed frame into a heap
owner. The retained-destination guard catches reference-counted escapes, while
borrowed waiter/source/callback addresses require the caller's lifetime proof.

`llg_co` is packaged in the runtime archive and self-contained source exports.
Generated processes execute through `llg_co` frames, descriptors, anchors and
arenas. Keep `llg_co` free of globals and TLS.
Frames are POD; callers own embedded callee frames and the prefixes of anchored
callees. Under `LLG_CO_DEBUG`, root storage and newly acquired embedded/arena
callee payloads use `LLG_CO_POISON_BYTE` (`0xA5`) before initialization/argument
transfer. The emitter poisons dead sibling-overlay structs after scope owners
are drained, including nonlocal named-block exits and loop breaks. Common-return
cleanup precedes completed-frame payload poisoning, covering early returns.
Flattened scopes sharing live parent storage are excluded. Retained fork/ref/NBA cells stay heap-backed;
poisoning their dead frame pointers never touches retained payloads. Never poison
registered owners before destruction or a suspended frame. The byte pattern
makes stale reads observable; it is not a general read trap. Poison macros emit
nothing in release; llg_co ABI 1 and process ABI 3 remain unchanged.
Cancellation drains runtime-owned scopes and releases arenas without resuming
coroutine code. No C local may remain live across a resume point. Place
root frames immediately after their `llg_proc_t` record at `LLG_CO_ROOT(ch)`,
without forced padding or alignment. Place
anchored frames at `LLG_CO_ANCHOR_FRAME(anchor)` and preserve the corresponding
alignment assertions.

Every `llg_arm_*` that takes a specification array (signal, dependency, event,
wait-source or expression-descriptor list, including nested dependency arrays)
copies it before returning and keeps no pointer into it: generated coroutines
pass those arrays as compound literals in the arm call
([emitter](../emit_c/AGENTS.md)). The declaration in `llg_rt.h` states the copy
contract; an arm that must retain an array or an output address (the
`wait_order` result) must say so there and the emitter must then keep it in a
frame field or registered cell.

`llg_rt.h` defines the exported OOM and bad-state hooks before including
`llg_co.h`; `llg_co.c` alone receives the matching `LLG_CO_HOST_ALLOC` compile
definition because it includes the library header directly. The runtime owns one
non-TLS chunk cache for the simulation thread, bounded by the named
`LLG_CO_CHUNK_CACHE_MAX_BYTES` tunable. Segregated free lists make geometric
arena reuse a constant-time head lookup while retaining exact-size matching.
The cache exposes cumulative allocation/reuse/free and byte counters and releases
all cached chunks at cleanup. MT-1 gives each worker its own cache; do not move
mutable cache state into `llg_co`.

Use typed queues for all IEEE regions and PLI control points: Preponed, Active,
Inactive, Pre-NBA/NBA/Post-NBA, Pre-Observed/Observed/Post-Observed,
Reactive/Re-Inactive/Pre-Re-NBA/Re-NBA/Post-Re-NBA, Pre-Postponed/Postponed.
Iterate design/reactive work to a fixed point before postponed output; Reactive
callbacks may enqueue another design iteration. The reactive set drains completely
before design work re-enters, and an assertion clock edge produced by the reactive
set returns the slot through Observed (SV 4.5, 24.3.1). Region callbacks scheduled
with `llg_schedule_region_callback_id` can be cancelled while queued; their
identity carries the runtime lifetime, so cancels from an earlier run are stale
no-ops. `#0` enters Inactive/Re-Inactive;
NBA/Re-NBA preserve issue order. Preponed/Observed/Postponed views are immutable;
illegal writes or read-only scheduling fail controllably.

Active coroutines are FIFO. Each initial/always/link/continuous process owns a
coroutine; ordinary forks use `llg_fork`, captured forks `llg_fork_with_frame`.
Every spawn/fork call supplies an immutable `llg_co_desc_t`. The runtime
co-allocates and initializes its root frame after the process record and owns one
arena until that record is reclaimed, including kill and teardown paths.
A completed top-level process (static, assertion action or detached spawn) is
retired at completion, or when its last join_none group detaches, and freed at
the next `reap_retired_procs` boundary; it can no longer be killed, so its
same-slot NBAs leave the cancellable owner list and commit from their queues.
Fork children are freed with their zombie groups; finals never retire.
Creators release frame references after spawn; children release on completion,
cancellation and teardown. Joined children may borrow live parent cells; cancel
children before releasing parent storage. Completed parents remain alive for
detached descendants. Process handles retain terminal identity separately from
coroutine storage. Normal process completion returns `LLG_CO_DONE`; terminating
services set `chain.exiting` and the generated continuation returns
`LLG_CO_EXIT` after unwinding its lexical scopes.

Program processes launch Reactive with program-instance origin. Count initial
procedures separately from fork descendants: last-initial completion cancels that
origin's detached descendants; all-program completion ends simulation. `$exit`
cancels its thread's originating program and returns harmlessly without such origin.

Future NBAs own values and retained destinations after issuers finish, and can
advance time without a process waiter. Masked writes merge into current storage.
Tagged-member NBAs own a copy of their tag checks and re-check the target's tags
at commit; a mismatch reports a runtime error and stores nothing.
Inertial sites own pending Active events: changed pending values cancel, unchanged
values retain deadlines, return-to-current cancels without replacement. Reset
model handles before freeing driver storage; reinitialization discards pending
events. Zero-delay drivers drain Active. Generated nets provide exact-size
driver interval nodes and overlap scratch storage. Selected writes replace their
previous interval, release it to Z when a selector moves, and resolve/publish only
the union of old and new ranges against overlapping drivers. Keep full-net waiter
identity and notification behavior. Pending delayed drivers start X, not driverless
Z. Fixed-array delayed sites retain per-element identity.

Opaque activation slots may hold event object identities, valid until model
teardown, or a parent coroutine's event handle address for a synchronous join.
Cancel joined children before releasing that parent frame. Detached event
captures own handle copies and never borrow a parent's handle address.
Waiters own snapshots/dependency lists. Packed edges use LSB, including 0→X/Z and
X/Z→1 posedges (negedge mirrored); real changes compare IEEE bits, including signed
zero and changed NaN payloads. `iff` executes at trigger. Event lists register
atomically, not as sequential waits. Wake/disable/teardown unregister every event
and free captures. Empty-dependency waits suspend without polling. Selected-prefix
wait masks ignore unrelated bits; evaluated callbacks remain read-only/effect-free.
Postponed display evaluators run with `private_evaluation`: packed, real and
fixed-array stores (lowering admits only helper-owned storage) apply without
publication; any other read-only-region write still fails. Event objects keep a
monotonic `trigger_count`; `llg_force_source_active` and
`llg_spawn_detached_with_frame` (no fork group, inherits origin) serve
process-evaluated effectful helpers.

`LLG_ZERO_LOOP_LIMIT` bounds region passes; `LLG_PROCESS_STEP_LIMIT` bounds generated
back-edges, with `LLG_NONCONVERGENCE_LIMIT` alias. Defaults are 10 million; explicitly
setting the region limit also supplies an otherwise unset process limit. Require
positive decimal uint64 values, rejecting invalid/overflow input before execution.
Source-located cooperative checks prevent non-yielding coroutines monopolizing
execution. Registries/queues grow with checked allocation, not historical fixed
ceilings; preserve stable identities across growth, reset grown event storage,
and reject `INT_MAX`/memory exhaustion without stale row pointers. `LLG_MAX_PROCS`
is only the standalone self-test's iteration base, not a scheduling ceiling.

## Stop, synchronization and assertions

`$finish`/deadlock end the scheduler. `$stop` instead yields a live suspended
context, preserving frames and same/future-time queues. `llg_rt_resume` requeues
at the same time. `LLG_STOP_POLICY=resume` (CLI default) avoids stdin waits;
`exit` returns the live context for inspection/resumption. Do not run finals while
suspended; generated CLI exit policy cleans up explicitly. Validate verbosity 0/1/2
without changing semantics. Normal completion/severity termination runs final
handling exactly once and preserves severity counters.

Semaphore waiters are FIFO. Cancelling a head or subtree must reservice surviving
waiters without an extra put; do not grant during an incomplete cancellation batch
or teardown. Mailboxes retain bounded/unbounded FIFO/peek rules and nominal typed
identity (including null handles), packed width/sign/state and real precision.
Mismatched try calls return -1 without assignment/consumption; blocking mismatches
fail. Remove consumed nodes before callbacks, clone peek values and register
ref targets/delivery snapshots through reentrant publication.

Clocking input samples complete before the Observed block event. Output captures
publish Re-NBA with constant skew; off-event drives wait for the next event and
`##N` counts events. `scheduler/dependencies.c` owns exact-descriptor edge history:
clocking synchronous drives and `##0` can query after publication, including after
registration in the same slot. Therefore publication records even automatic
locals without an existing waiter; subscriber absence does not prove an entry
unusable. Generated clocking-source emission currently rejects automatic bindings,
but native runtime callers can pass any live packed owner to the borrowed-source
APIs. Globally filtering that storage class would change accepted runtime behavior.
Ordinary waits use their own snapshots, input skew sampling uses
`sampling.c`, and named-event `.triggered` uses event-object state.
`llg_sampled_register_value` keeps only a signal's Preponed value (procedural
`$sampled`, every concurrent-assertion read) in a separate list, so it adds no
history and no per-write lookup. `llg_sampled_register` adds Observed clocking
copies; `llg_sampled_register_history` keeps the slots the deepest declared input
skew can select (the newest at or before `now - ticks` and everything newer),
promoting an earlier value-only entry. Sampled-value domains likewise keep only
their registered `$past` depth plus the current step. Never retain history for
an undeclared reader: memory must not grow with simulated time.

Slot history retains any/positive/negative occurrence independently, so a later
opposite edge cannot erase an earlier match. Time advance drops all entries except
registered sequence clocks, clears their occurrence flags and compacts the pointer
index. Sequence registration marks the root and explicit transition clocks before
execution; only those consumers need cumulative saturating directional counts for
same-clock delays across slots. Disabled assertions may later resume, so their
clock counts last until cleanup, independent of attempt lifetime. No count resets
at time advance. Cross-clock replay history in the assertion itself is separate.
Packed scope final release forgets every descriptor before freeing it; lexical
exit alone cannot forget a cell retained by queued writes. External packed owners
must call `llg_clocking_forget_signal` before freeing/reusing a written descriptor,
and must separately satisfy wait/sample/assertion source lifetimes. Model storage
survives runtime cleanup. The history list owns nodes; its hash index borrows them,
uses exact pointer equality, and must be updated on removal/reset/teardown. This
private layout/additive API leaves process ABI 3 and value ABI 4 unchanged;
embedded runtime-content hashing invalidates earlier cache archives.

Deferred immediate assertions keep issue-time sampled values;
Reactive refs resolve at action time, coalesced per process. OFF prevents new
checks but does not cancel pending actions; KILL/flush does. Drain actions before
cleanup.

Sequence tokens own a transition, local snapshot and retained first-match scope
chain. Each scope belongs to an invocation; completion cannot discard outer
siblings/suffixes. Preserve all earliest tied endpoints. Empty-word alternatives
use `admits_empty`/normalized concatenation, not current-tick epsilon; negative
repetition guards use four-state truth. Consequents inherit locals by declaration
ID, initializing only private cells. Common discard paths release snapshots on
abort/shutdown. Cross-clock ##0 uses nearest at-or-after physical time, ##1 strictly
later time; retain current-slot edge history for all consumers. Callback order is
only a replay guard, never elapsed cross-clock time. Explicit action choice must
not erase assertion failure accounting.

## I/O, VPI and waveform

Keep hosted C descriptor namespaces separate: FD bit 31 with preopened standard
streams, MCD bit 0 stdout and bits 1..30 channels. Fan-out applies only to MCDs.
Cancel deferred users before closing/reusing descriptors. Typed scanners validate
suppressed assignments and leave unmatched delimiters unread; targets are borrowed
for calls. `$system` requires explicit `LLG_ALLOW_SYSTEM`; retain host C status and
distinguish omitted system(NULL) from an empty command.

`scheduler/output_files.c` reads run-time output settings at every init:
`LLG_SIM_OUT_DIR` (created if missing) prefixes relative write paths only —
waveform, `$fopen` w/a modes, `$writemem*`, the log — never reads;
`LLG_SIM_WAVE_FILE` replaces `$dumpfile`/`dump.vcd`; `LLG_SIM_LOG_FILE` starts a
once-per-process forked tee (POSIX only; Windows reports an error) that survives
simulator crashes, shares one pipe when stdout/stderr reach the same file, and is
joined at exit. `llg_wave.c` reads `LLG_SIM_OUT_DIR`/`LLG_SIM_WAVE_FILE` itself in
`llg_wave_model_init`, keeping the waveform unit scheduler-independent.
`llg_wave_start` (generated for `llg --wave`) records its file as the override
unless `LLG_SIM_WAVE_FILE` set one, opens it and selects at time zero after all
registrations; afterwards `$dumpfile`/`$dumpvars` are ignored while the other
dump controls still apply.

Memory images preserve X/Z, comments, addresses, views and loaded prefixes on
failure. Keep edition-specific default address order, explicit direction, short-word
padding, truncation/original-width enum validation and sparse-file diagnostics.
For the admitted signed `@` extension, check hexadecimal magnitude against the
signed index range before applying the sign; leading zeroes cannot affect the
index, and selected-view bounds must be checked before writes.
Binary rank-one `$fread` advances low-to-high HDL addresses independently of physical
declaration order, clamps count at the high bound, and consumes/writes nothing for
invalid/unknown bounds or negative count. Keep element owners across callbacks;
this does not qualify multidimensional binary reads.

Monitor registration/enabling forces a postponed report; only registered signal
changes dirty later reports, and only the latest monitor is active. Strobes wait
for settled NBA-triggered work. `%t` retains caller units; design-wide timeformat
never changes scheduler time. Integer queries use quotient/remainder, halves up;
realtime is fractional. The runtime consumes integer design-precision ticks;
codegen owns scaling.

VPI handles are generation-checked; retain error records, real catalog width 0 and
simulator-owned vector scratch valid until the next query/shutdown. Call-argument
iterators carry owner identity and invalidate at every compiletf/sizetf/calltf exit.
Tear down VPI/runtime queues before model storage.

Waveforms use one simulation-thread producer and one POSIX/Win32 writer owning the
file. Ring events own values; release/acquire publication, condition-variable
backpressure, acknowledged FIFO dumpflush and in-band close followed by join are
required. Each ring-index or waiting-flag store is followed by a sequentially
consistent fence before reading the peer's flag or index; release/acquire alone
loses wakeups and deadlocks close/dumpflush. A second producer needs a separate ring or changed concurrency design.
Separate hierarchy components with ASCII 0x1f and reversibly encode punctuation to
avoid escaped-name collisions. Keep libfst internal parallel mode disabled;
waveform builds require `Threads::Threads`, not C11 threads as the Windows
boundary. They compile only the needed `vendor/zlib` files from
`waveform_sources()` under `zlib/` with `Z_PREFIX`; never reintroduce
`find_package(ZLIB)` or a system zlib link. Preserve GTKWave provenance and
official-reader self-tests.

## Source assembly and validation

C11 model builds use `-O2 -Wall -Wno-unused-function` and stay warning-clean;
generated model sources add `-Wno-misleading-indentation` (see the sim guide).
Compile facade translation units only; synchronize private fragment order with
`include_str!`/`concat!` flat embedding. Never include value implementation in the
scheduler or export private shared state to avoid assembly rules.

Preserve source APIs: `value_sources`, `random_sources`, `rng_sources`,
`coroutine_sources`, `runtime_sources`, `string_sources`, `container_sources`,
`selftest_source`, `selftest_support_source`, `waveform_sources` and
`waveform_selftest_source`. Value/random/RNG/llg_co are independent;
runtime/string/container require values, container not scheduler. Models
initialize through `llg_rt_init_with_args_and_precision` and define
`LLG_MODEL_PROCESS_ABI`.

Keep the generated 256-call guard: recursion runs on heap frames, but the
guard still bounds plain-function nesting, which the host-stack estimate
assumes. POSIX startup warns below the named 640 KiB estimate
(`LLG_HOST_STACK_MEASURED_BYTES` plus `LLG_HOST_STACK_FOREIGN_HEADROOM_BYTES`);
generated MSVC model targets reserve at least 1 MiB with `/STACK`. User DPI/VPI
C code shares the stack and is covered only by the headroom. Stackless sanitizer runs use ordinary ASan/UBSan flags without a
stack-switch exception.

Use [runtime tests](../../../tests/runtime_value_storage/readme.md) for component,
ABI, allocation-failure and waveform checks, and
[repository validation](../../../tests/AGENTS.md) for integration. Preserve native
Windows qualification and generated-model integration remain separate gates; the
stackless process and cancellation probes run under ASan/UBSan.

Shared-body startup uses `llg_spawn_instance_in_region` or its program variant.
The record is immutable model-lifetime data. Before the scheduler can run the
queued process, these services copy its pointer into the generated root payload
at the supplied `offsetof`, checked against the header and descriptor frame size.
Existing spawn services remain valid for roots without an instance-record field.
No record ownership or cleanup transfers to the runtime.
