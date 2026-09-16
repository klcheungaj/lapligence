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

The scope owns initialized descriptors. Normal end unlinks and destroys it;
process completion, disable/kill, retirement and runtime cleanup unwind remaining
registered scopes. Coroutine stack unwinding is not required. Scope pointers and
values are invalid after end/cancellation. Do not manually end a canceled scope,
move descriptors out without clearing them, or link owners to another process.
Short-lived expression scopes must end during long loops, not only on process
termination. Root scopes are unwound at runtime teardown/reinitialization.
`$stop` must preserve scopes until resume or explicit cleanup.

Plain locals used only within a non-suspending runtime helper use one cleanup
path or explicit destroy on every return. Cross-yield/runtime callbacks must not
retain a pointer to a local descriptor; capture or move into an owning object.

## Retained-state destruction map

| Owner | Construction/capture | Release boundary |
| --- | --- | --- |
| NBA and clocking records | Deep-copy value and mask | Commit, cancellation, cleanup/reinit |
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

Descriptors for model targets, registered object addresses and callback code are
borrowed, not made immortal by these captures. Generated model/global/activation
initialization and lifetime must be completed before restoring emission.

## Staged integration boundary

P03/P04 changed the main representation early rather than retain a fixed-size
compatibility layer. The P05 expression emitter still emits untracked temporaries
and static literals. Public C emission and build entry points therefore fail with
an explicit migration error; legacy selftests are fenced until their fixtures
are migrated. Do not bypass those checks or compile stale generated C against
this runtime. P06 still removes obsolete emitter/cache/stack sizing metadata.

The component tests establish exercised behavior, not a proof of all paths.
Full HDL execution, exact Rust/C parity, native macOS/MSVC and performance
measurement are later acceptance gates. See the standalone tests and delivery
validation report for the checks actually executed.
