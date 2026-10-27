# Runtime scheduler

## Frame-resident cells

Process ABI 3 exposes the scope-node layout and adds caller-owned packed/native
registration. Scope
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

The emitter's [typed proof](../../emit_c/storage.md#storage-references-and-publication)
excludes the following retainers or supplies the indicated cleanup:

| Descriptor retainer | Lifetime rule for intrusive locals |
| --- | --- |
| `nonblocking.c` NBA/selected/masked/future/event-controlled writes and `dependencies.c` pending clocking drives | NBA target locals remain heap-backed; clocking/event captures are unproven. |
| `activations.c`, `reference_writes.c`, queue refs, output/inout/ref and foreign call descriptors | All address actuals remain heap-backed; reference descriptors themselves retain their existing heap scopes. |
| `storage.c` capture slots and `forks.c` joined/detached descendants | Captured forks and spawning callee effects fail the proof. |
| Wait snapshots, R2 subscriptions, expression/iff contexts and `wait_order` delivery | Local wait dependencies and context-free evaluator reads remain heap-backed; captured evaluator/qualifier contexts are unproven. Existing wake/cancel paths remove subscriptions before scope unwind. |
| R1 `dependencies.c` clock history | Every final scope release forgets the exact packed descriptor before reuse; publication without a waiter is still recorded. |
| Clocking sources, sampled histories, concurrent assertions, sequence clocks/local scopes | Clocking/sampled/assertion forms fail the proof; external owners retain their documented lifetime obligations. |
| `nets.c` inertial handles/targets, `force.c` targets/evaluators | These registrations fail the proof; model-lifetime sources keep their existing path. |
| `mailboxes.c` delayed delivery, pinned targets and reentrant publication snapshots | Mailbox/native service operations fail the proof. |
| VPI catalog/callback/call handles, DPI/foreign pointers and dynamic dispatch | These operations fail the proof; no frame cell is exposed through them. |
| Monitor/strobe/output callbacks and deferred assertion reports/action frames | Deferred readers/capture graphs fail the proof; legal persistent sources remain model-owned. |
| Synchronous write `value_target_pin` | The pin ends before the write returns. Proven locals have no waiter/foreign callback observer that could retain the target beyond scope end. |
| Container/object/alias/net and waveform registrations | These forms fail the proof. Ordinary by-value packed/string reads clone payloads; waveform events and formatting buffers own independent snapshots. |

Unknown typed operations fail closed. Native callers must separately meet the
same retainer obligations; registration never turns a borrowed frame into a heap
owner. The retained-destination guard catches reference-counted escapes, while
borrowed waiter/source/callback addresses require the caller's lifetime proof.

## Coroutines and arming

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
Cancellation and the no-C-local-across-resume rule: `llg_co.h` header. Place
root frames immediately after their `llg_proc_t` record at `LLG_CO_ROOT(ch)`,
without forced padding or alignment. Place
anchored frames at `LLG_CO_ANCHOR_FRAME(anchor)` and preserve the corresponding
alignment assertions.

Every `llg_arm_*` that takes a specification array (signal, dependency, event,
wait-source or expression-descriptor list, including nested dependency arrays)
copies it before returning and keeps no pointer into it: generated coroutines
pass those arrays as compound literals in the arm call
([emitter](../../emit_c/evaluation.md)). The declaration in `llg_rt.h` states the copy
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

## Regions and processes

Use typed queues for all IEEE regions and PLI control points: Preponed, Active,
Inactive, Pre-NBA/NBA/Post-NBA, Pre-Observed/Observed/Post-Observed,
Reactive/Re-Inactive/Pre-Re-NBA/Re-NBA/Post-Re-NBA, Pre-Postponed/Postponed.
Fixed-point iteration: `llg_rt.h` header. The reactive set drains completely
before design work re-enters, and an assertion clock edge produced by the reactive
set returns the slot through Observed (SV 4.5, 24.3.1). Region-callback cancellation: `llg_schedule_region_callback_id` docs. `#0` enters Inactive/Re-Inactive;
NBA/Re-NBA preserve issue order. Postponed reports run monitors first, in
registration order, then strobes, the last issued first (S25-D6); a slot with
neither pays nothing. Preponed/Observed/Postponed views are immutable;
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
coroutine storage: counted storage (process variables, by-value formals,
container elements) retains and releases; plain handle storage (class
properties, record leaves, non-input formals, results, fork-shared slots)
holds `llg_process_pin`ned handles, whose pin `llg_rt_cleanup` drops. Process
methods through a null handle and `await` on the caller are fatal. Normal process completion returns `LLG_CO_DONE`; terminating
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
values retain deadlines, return-to-current cancels without replacement. Driver
delay selection: `inertial_transition_ticks` comment (`nets.c`). A net delay keeps one
record for the whole resolved vector, or one per separately driven part
(`segment_lsb`/`segment_records`), and the new value of the record's least
significant bit selects the delay (`net_delay_ticks`, S2-D3); a runtime-valued
net delay (`net_delayed == 2`) takes the whole-value vector rule instead (S2-D9). A driven
delayed net or path destination starts X with a strong-X strength view
(`llg_net_start_unknown`, S2-D10) and reaches its resolved value through its
delay. An undelayed driven wire, wand or wor holds X (strong-X view) on its
driven bits (`llg_net_start_driven`, T0-D3) until the first write of a
driver covering them (`unsettled`; one byte test per write). With several
drivers, `start_pending` keeps one bit per still-unwritten driven bit; while
it is set every write resolves the whole net and those bits stay X (StX), so
one driver's first write never exposes another driver's bits as z. A write
covers the whole net, a selected write its range, an alias write its mapped
bits and `llg_net_write_driven` (startup fills) its non-z bits. A
runtime-valued net delay is the generated updater's store into
`propagation_rise/fall/turn_off`; each later publication reads those fields and
pending records keep their times. Reset
model handles before freeing driver storage; reinitialization discards pending
events. Zero-delay drivers drain Active. Deferred multi-slot net resolution (S2-D11):
"Deferred net updates" comment in `nets.c` (`llg_net_defer`,
`deferred_slot`). Writes outside a process turn (inertial
commits, startup, callbacks) resolve at once. Generated nets provide exact-size
driver interval nodes and overlap scratch storage. Selected writes replace their
previous interval, release it to Z when a selector moves, and resolve/publish only
the union of old and new ranges against overlapping drivers. Keep full-net waiter
identity and notification behavior. Pending delayed drivers start X, not driverless
Z. Fixed-array delayed sites retain per-element identity.

## Switches, module paths, timing checks and SDF

Pass-switch networks (`scheduler/switches.c`, ADV-005) are model-lifetime
state built by `llg_switches_start` from the generated node and switch
tables: CSR adjacency, per-net member lists (`llg_net_t.switch_net`, NULL for
every other net, so unswitched nets pay one pointer test) and four strength
levels per node (strongest known/possible 0 and 1). A driver-slot commit,
force or release of a member net marks its nodes dirty and queues the net
instead of resolving it; `llg_switch_control` marks a switch's ends. Direct
controls (`llg_switch_controls_start`) watch their signals' storage through a
waiter-less wait-source subscription (`llg_wait_source.switches`, like timing
checks); a publication compares the stored words with a per-signal shadow and
updates only the switches of changed bits. Each node's adjacency lists its
non-off switches first (`n_active`, swapped in O(1) on a state change), so a
solve never visits off switches. A flush round that a publication re-dirties
counts against the zero-delay guard (`LLG_ZERO_LOOP_LIMIT`). The
solve of a dirty dynamic component (nodes joined by switches that are not
off) is an 8-bucket strongest-first propagation: `tran*` keeps strength
except supply to strong, `rtran*` uses Table 28-8, an unknown switch passes
possible levels only (the union of A5-D1), a level leaves a node only if no
stronger opposite known level holds it (A5-D2), and a forced bit is a fixed
strong source (A5-D3). Each node entry rises at most once, so a solve is
O(nodes + switches) with no iteration cap. Changed nets republish in queue
order through `llg_net_resolve`, whose compute and strength paths overlay the
member bits with the network result. Inside a process turn the solve waits for
the region drain like S2-D11 (`llg_net_flush_deferred` flushes it); the first
solve waits for the first region that may update nets (T0-D2).

Module paths (`llg_module_path_set_t`, one per instance with paths) are laid
out by `llg_module_paths_layout` from an immutable per-module
`llg_module_path_template_t` (rows, destination paths, styles, limits,
condition evaluator) plus the instance's source and destination references,
copying the template rows and limits into the model's writable pools in set
order (SDF annotation then edits those copies). The set's `condition_context`
is passed to its condition evaluator, so one evaluator serves every instance.
They borrow the model's source storage and own, between `llg_module_paths_start`
and `llg_rt_cleanup`, a last value per source and a last-change time per
source bit. A bound destination net (`llg_net_t.path_dest`, NULL otherwise, so
undelayed nets keep their single flag test) always has `propagation_enabled`
and per-bit path-stage records (`propagation_bits`): `llg_net_propagate_paths`
first refreshes the stamps (order-independent within a slot), then schedules
each changed bit at `max(now, latest input time + smallest path delay)` over
the enabled paths whose input time is latest (A7-D1, A8-D1): conditions come
from the set's generated evaluator, read lazily once per publication; edge
paths use per-bit rise/fall stamps (allocated only for sets with edge paths);
an `ifnone` path yields to an active path of its group. `path_bit_schedule`
then filters pulses (30.7): the record's pending event is the head and further
events live in its `llg_path_queue_t` (allocated on the first passed or
x-filtered overlap, freed at cleanup); reject/error limits come from the path's
`limits` table or the `llg_module_paths_configure` percentages; styles from
the destination flags or the configured overrides. With default limits no
queue is allocated and the stage is the ADV-007 inertial update.
`commit_inertial` promotes the next queued event after a path-stage commit; a
force clears the queue. A matured path stage publishes directly, or, on a net with a
net delay (`net_delayed`), feeds the net-delay stage above, so the net delay
adds to the path delay. Delay tables
and limit tables are writable for annotation; `llg_net_path_pending` and
`llg_net_path_event` report scheduled path events. A force or release resynchronizes the propagation records'
committed values (`llg_net_propagation_sync`), so the next delayed
transition starts from the published value.

Timing check sets (`scheduler/timing_checks.c`, `llg_timing_check_set_t`, one
per instance with checks) are laid out the same way by
`llg_timing_checks_layout` from an `llg_timing_template_t` (terminals, checks,
delayed-signal lines, condition evaluator) plus the instance's terminal,
notifier and line-net references and `condition_context`. They borrow the
model's terminal storage and own, between
`llg_timing_checks_start` and `llg_rt_cleanup`, a 0/1/x state per terminal
bit, per-step transition masks, condition values and notifier flags; each
check row keeps its timestamps (0 is none: time-0 transitions never count,
A9-D3), the delays of the stamped transitions and `$nochange`/skew window
state. The watch/doorbell/step flow is the `timing_checks.c` header comment
(doorbell: `llg_timing_checks_doorbell`). Details it leaves out: transitions
treat z as x and x<->z as none; delayed signals (`llg_timing_line_t`) cancel a
pending transition that a new one lands before, use one Active-region callback
for the earliest due transition and write through the line's net driver slot
or as a variable; conditions evaluate lazily once per set; rows reading a
changed terminal run in check order with limits adjusted by the stamped delays;
reports print undelayed times on stderr unless `llg_timing_checks_configure(0)`;
each violated notifier toggles once per step with `llg_ba`. Skew windows live
in one heap (stale entries are skipped by deadline);
`llg_timing_check_limits_changed` re-arms them after an annotation.
Limits are writable for annotation; steady-state steps allocate nothing
(deferred `$nochange` data and pending delay transitions grow on demand).
A row flagged `LLG_TIMING_DISABLED` never reports; a report text may carry
`\003`/`\004` placeholders that print the row's current `limit`/`limit2`.
`llg_timing_violation_hook` installs one function called after each report
with the set and check (VPI `cbTchkViolation`); it is NULL unless a callback
is registered, so a violation then costs one predicted branch.

SDF annotations (`llg_sdf_annotation_t`; model in the `scheduler/sdf.c`
header): `llg_sdf_apply` writes path delays and pulse limits (allocating a
path's limit table on first use, deriving limits from the configured pulse
percentages and clamping them to the delay), gate delay triples
(`llg_sdf_gate_delays`), interconnect load entries combined by the op's
INTERCONNECT_MIPD policy into the load's interconnect path (a
`llg_module_path_t` with `.interconnect`, whose input time is the current
time), and check limits; a set with annotated combined checks
(`llg_timing_pair_t`) is settled and its delayed-signal delays solved again
(the codegen settlement and Bellman-Ford solve), disabling rows whose window
is shorter than two units. Startup
annotations run in the storage defaults before the path sets, delayed nets
and timing checks start; `sdf_release` frees the allocated limit tables at
cleanup.
A path op flagged `LLG_SDF_KEEP_LIMITS` (VPI `vpi_put_delays` without
`pulsere_flag`) first materializes the path's limit table from the current
delays, so the new delays leave the pulse limits as they were (38.32);
`llg_module_path_pulse_limits` reads the limits a transition uses.

## Activations, waits and limits

Opaque activation slots may hold event object identities, valid until model
teardown, or a parent coroutine's event handle address for a synchronous join.
Cancel joined children before releasing that parent frame. Detached event
captures own handle copies and never borrow a parent's handle address.
Waiters own snapshots/dependency lists. A suspended process's event-control
wait (`W_EVENTS*`, `W_EVENT`, `W_MIXED`, `W_EXPR`, and `W_DEPS` armed by
`llg_arm_event_dependencies`) withholds occurrences (`wait_held_by_suspension`)
while its snapshots stay current, so `resume()` resensitizes it (SV 9.7); wait
conditions, delays and other blocking calls keep a pending wake. Semaphore
and mailbox get/peek/put waiters are withdrawn from their FIFO on `suspend()`
(`wait_queue_withdraw`) so keys, messages and space stay with other waiters,
and `resume()` re-queues them at the tail and services the queue
(`wait_queue_rejoin`); the registration and payload stay live for kill and
teardown. A fork branch whose whole body is one named block exits it with
`llg_activation_exit_terminal`; if a disable ended the block, `proc_complete`
records `KILLED` instead of `FINISHED` (`disabled_whole`, SV 9.7). A
process-evaluated plan marks its wait with `llg_wait_refresh_on_resume`, and a
withheld occurrence wakes it on resume with `llg_wait_refreshed()` set so it
re-arms its values without detecting a change. Packed edges use LSB, including 0→X/Z and
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

Zero-time limits (`LLG_ZERO_LOOP_LIMIT`, `LLG_PROCESS_STEP_LIMIT`, alias,
validation): `llg_rt.h` header comment. Explicitly setting the region limit also
supplies an otherwise unset process limit; cooperative checks are source-located. Registries/queues grow with checked allocation, not historical fixed
ceilings; preserve stable identities across growth, reset grown event storage,
and reject `INT_MAX`/memory exhaustion without stale row pointers. `LLG_MAX_PROCS`: `llg_rt.h` comment.
