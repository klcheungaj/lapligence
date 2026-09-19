# Packed-value ownership

## Representation and lifetime

`sv4_t` is a unique owner, not a trivially copyable value. One checked allocation
contains contiguous `bits`, `x`, and `z` planes, each `ceil(width / 64)` uint64_t
limbs. Only `bits` is freed. The payload is exactly `24 * ceil(width / 64)` bytes;
width zero has no allocation. Width is less than `LLG_SUPPORTED_WIDTH_LIMIT`
(currently 1,048,576), with no model-width cap. Partial top limbs are masked.
Signedness is metadata, not a reason to allocate another limb.

Every destination starts with `SV4_EMPTY`, zero initialization, or a constructor.
Every value-returning operation returns an independently owned result, including
same-width resize/cast, known-arm mux, identity operations, reference reads and
container gets. Packed by-value input parameters borrow for the call unless
explicitly documented as consuming. Ordinary structure assignment is permitted
only as a transient borrow or an explicit transfer whose old owner is reset.
Never destroy a borrow, or retain its interior pointers across mutation/yield.

| API family | Input/output ownership |
| --- | --- |
| `sv4_zero`, `sv4_from_*`, `SV4_C/S/X/Z/INIT` | Construct a fresh owner; macros are runtime calls, never static initializers. Raw limb inputs are borrowed. |
| All arithmetic, predicates returning sv4_t, conversions returning sv4_t, resize, cast, selection, enum, resolution, stream and string-to-packed results | Borrow packed arguments, return a fresh owner. String arguments retain their separately documented consuming convention. |
| Scalar inspectors, comparison helpers, packed-to-real, formatting | Borrow packed arguments, return scalars or fill caller-owned buffers. |
| `sv4_clone`, `sv4_copy`, `sv4_assign` | Deep clone. Copy/assign replace an initialized destination; allocation precedes destruction. Copy supports self-copy. |
| `sv4_move` | Destroy initialized destination, transfer source allocation, reset source. Self-move does nothing. |
| `sv4_replace` | Consume one fresh returned owner. Use `move` for named owners so the source is reset. |
| `sv4_destroy`, `sv4_destroy_array` | Destroy live owners and reset descriptors. Destruction of an empty value is safe. |
| `sv4_*_set` | Mutate an initialized destination, borrow RHS. Selected-write implementations snapshot aliased RHS before mutation. |
| `llg_ref_read`, packed container getters/reductions/pops | Return an independent owner; neither a retained alias nor a borrowed element. |
| Container setters/copies, scheduler writes, NBA/clocking/inertial enqueue | Borrow inputs; retained payloads are cloned. Destination lifetime is a separate caller obligation. |

Passing a fresh returned owner to a **borrowing** operation does not destroy it.
Do not write `sv4_add(sv4_zero(...), sv4_zero(...))`: name both operands and destroy
them, or store them in a registered scope. No simulation-lifetime arena or hidden
reference count excuses an untracked copy. `llg_string_*` retains its existing
consume-string convention; do not apply packed borrowing rules to string inputs.
Typed formatting argument arrays own packed and string members and are consumed
by formatting/output calls. Mailbox put operations consume their tagged payload.

## RAII-style containing objects in portable C11

C11 has no automatic C++ destructor on scope exit. The runtime therefore pairs
construction with deterministic containing-object destruction and provides a
registered heap scope for owners that must survive coroutine suspension. This
uses no cleanup attribute, VLA, alloca, statement expression or C++ ABI.

```c
llg_value_scope_t* scope = llg_value_scope_begin(3);
sv4_t* values = llg_value_scope_values(scope);
sv4_replace(&values[0], sv4_from_u64(7, 65, 0));
sv4_replace(&values[1], sv4_from_u64(9, 65, 0));
sv4_replace(&values[2], sv4_add(values[0], values[1]));
llg_ba(target, values[2]); /* borrows; target must already be initialized */
llg_value_scope_end(scope);
```

The scope owns initialized descriptors. Normal end unlinks it and releases its lexical reference;
process completion, disable/kill, retirement and runtime cleanup unwind remaining
registered scopes. Coroutine stack unwinding is not required. Scope pointers and
values are no longer available to the caller after end/cancellation, even when
a runtime record still retains the underlying cell. Do not manually end a canceled scope,
move descriptors out without clearing them, or link owners to another process.
`llg_value_scope_mark` and `llg_value_scopes_end_since` delimit generated
lexical regions without requiring C stack unwinding; a non-null mark must remain
an active ancestor in the same process/root scope chain. Do not manually end a
scope serving as a live mark. Temporary payloads must be destroyed during long
loops, not only on process termination; their empty descriptor slots may be reused. Root scopes are unwound at runtime teardown/reinitialization.
`$stop` must preserve scopes until resume or explicit cleanup.

Plain locals used only within a non-suspending runtime helper use one cleanup
path or explicit destroy on every return. Cross-yield/runtime callbacks must not
retain a pointer to a C stack descriptor; capture or move into an owning object.
Ordinary packed NBA and clocking enqueues recognize registered scoped target
cells and retain their containing scope. Those records release that reference
on commit/discard and preserve it across clocking-to-NBA transfer. A separate
registry covers scopes detached by lexical exit or process retirement. It never
uses relational comparisons between unrelated C pointers. Other callback/force/
reference families are not implicitly pinned by this facility; their emitter
paths remain gated until their retention contracts are migrated.

## Retained-state destruction map

| Owner | Construction/capture | Release boundary |
| --- | --- | --- |
| NBA and clocking records | Deep-copy value/mask; retain scoped packed target descriptor when present | Commit, cancellation, cleanup/reinit |
| Signal write notification | Independent old-value snapshot | After notification returns, including unchanged paths |
| Inertial drivers / force state | Deep-copy scheduled value/mask/current/baseline | Replacement, canceled update, commit, driver/force cleanup |
| Waits and sampling history | Deep-copy snapshots | Wake/rearm, process cancellation, history expiry, cleanup |
| Activation frame / registered value scope | Owned initialized cells or explicit reference aliases | Frame release, lexical scope end, completion/cancellation/cleanup |
| Sequence attempts, tokens, endpoints | Deep-copy inherited local values | Dedup/discard, attempt/endpoint destruction |
| Mailboxes | Tagged payload construction/transfer | Consume, failed/canceled put, mailbox/runtime destruction |
| Dynamic arrays, queues and associative entries | Deep-copy packed/recursive elements, keys/defaults | Replacement, resize/delete/pop, container destruction |
| Pinned queue cells | Owned snapshot with reference count for cell identity | Last pin release, after disconnecting removed cells |
| Waveform events | Independent clone; publication transfers owner | Writer common cleanup, ignored/error events, queue drain/close |
| VPI cached calls | Initialized owned return and compile arguments | Callback replacement, call-site release and shutdown |
| Typed monitor/output buffers | Initialized evaluated/captured owners | Print/discard, next evaluation, cancellation/teardown |

Descriptors for model/global targets and callback code remain borrowed, not made
immortal by value capture. Scoped packed NBA/clocking targets are the explicit
exception described above. Model close releases runtime queues/VPI state before
freeing persistent model owners.

## Staged integration boundary

P03/P04 changed the main representation without a fixed-size compatibility layer.
P05 now has a structured numeric whole-model path, registered expression/local
lifetimes and explicit model start/advance/close. Unmigrated features and all
legacy fragment APIs remain fail-closed; the original C selftests now use explicit owners. The new
Rust emitter was not compiled in this delivery environment. See
[exact emitter coverage](../../emit_c/owned/readme.md).

P06 replaces active model-width allocation/build metadata with an explicit
ownership ABI (3) and width-independent runtime cache. Generated models include
a compile-time ABI assertion; source builds validate their marker. Regenerate
model and runtime together. This does not establish full model integration.

The component tests establish exercised behavior, not a proof of all paths.
Full HDL execution, exact Rust/C parity, native macOS/MSVC and performance
measurement remain acceptance gates. Hand-authored C output-shape probes are
not Rust-emitted-model tests. See the delivery validation report for executed
checks and their limitations.

## Registered native payloads and publication callbacks

`llg_value_scope_begin_object(bytes, destroy)` allocates zero-initialized, stable
native payload storage; `llg_value_scope_object(scope)` borrows it. The supplied
destructor releases payload contents only. It must not free the payload itself,
yield, or publish simulator notifications. Scope teardown invokes the destructor
and then frees the payload. Such scopes share the ordinary lexical/process/root
unwind stack, but are not packed NBA destination descriptors. A string owner uses
`llg_string_destroy`; a process-handle owner releases its retained handle.

`llg_string_take` moves an expression owner's descriptor and clears the source.
It is not a write to an HDL string variable: writes use `llg_string_move` to preserve
destination dependency metadata. Both returned expressions and callee input copies
must be owned, not shallow aliases. Native return slots precede call cleanup marks.

`llg_queue_pop_front_into`/`llg_queue_pop_back_into` adopt the removed packed value
into the caller's registered result slot before notifying readers. Converted generic
queue/associative inputs, default values and normalized keys are released before
notification, while newly installed elements remain owned by the container. Mutation
helpers report change flags to their public wrappers; notification stays synchronous
and preserves no-change suppression, but occurs only after wrapper cleanup. The
container runtime remains scheduler-independent.

Reference writes, scanner tokens/values and dependency-marker updates may span
callbacks and therefore use registered owners. File line buffers and plusarg format
pieces are freed before destination publication. A nonreturning callback must never
be the only reason a runtime-local owner is abandoned without cleanup.

## Addressable real locals and exact native destination pins

Addressable automatic real variables use individual registered `double` payloads.
A pointer to a plain C stack local is not a persistent mailbox destination on
libaco's shared stack: another coroutine's write can be overwritten when the
receiving coroutine's saved stack is restored. Real arithmetic temporaries that
are not retained by another activation may remain C scalars.

The private exact-address index includes both packed descriptors and native
payload base addresses. It does not index interior native fields or retain every
address reachable from a payload. Packed and real signal publication use a
registered pin on the writer while callbacks run. If a callback cancels the
receiving process, the destination stays alive until publication completes; if
the writer exits nonlocally, its pin is unwound. Global/model storage remains
owned by the model. These pins do not add support for queued automatic real or
string writes: their emitter guards and queued-write contracts remain unchanged.

A selected reference write passes its already captured `uint64_t` index to
`llg_ref_write_bit`; it does not wrap that index in an owning `sv4_t` descriptor.

## Packed selection plans

`sv4_select_plan_t` is an owner-free coordinate descriptor. Start with
`sv4_select_plan_init(storage_width)` and apply borrowed integral bases through
`sv4_select_plan_step`. Each step intersects with the preceding valid interval;
invalid prefixes cannot become valid again through later offsets. Unknown or
unrepresentable bases produce an empty valid interval. Width/shape mismatches
remain controlled fatal runtime errors.

`sv4_select_plan_read` borrows the whole source and returns an independent unsigned
packed owner with X in invalid positions. `sv4_select_plan_set` borrows both the
plan and RHS; it snapshots an aliased RHS before modifying the destination and
writes only the valid interval. All payload allocation follows actual widths.
Neither operation publishes scheduler notifications. The scheduler/emitter owns
publication after values are installed.

The additive `LLG_REF_PACKED_PLAN` tag is reserved for synchronous scanner/input
operations; its `retained` field borrows a plan rather than owning a queue cell.
Do not pass it to retained-reference scope cleanup or keep it after its input
call. Existing `sv4_t` and `llg_ref_t` layouts and ownership ABI version 3 are
unchanged. Regenerate models with the matching runtime sources.
