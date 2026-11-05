# Emission: storage, native boundaries and captures

## Storage, references and publication

`owned/frame_cells.rs` proves local descriptor eligibility from typed IR and
callee effects before emitting any procedure storage. A non-escaping packed,
real or native local registers an intrusive `llg_value_scope_t` and an empty
value cell in the coroutine frame (including zero-resume procedures and
resume-free inner blocks), or in the outer C scope of a frame-less function. Required frame fields
remain live through common-return unwinding; sibling overlays may reuse them
only after all registrations in the preceding scope have ended. Initialization
and registration still occur at the original declaration, preserving LIFO cleanup.

NBA destinations, all address/ref/output call actuals and local wait dependencies
keep heap cells. Typed binding names identify cells; never infer eligibility from
generated C. Unknown expression/statement forms fail closed for the whole
procedure. Context-free read-only event evaluators qualify only after scanning
their typed reads and wait dependencies; observed cells remain heap-backed.
Captured evaluator contexts, fork captures, clocking, sampled/assertion operations,
force/inertial registrations (whose fixed-array sources register element or
contents change markers), mailbox delivery, VPI, DPI and dynamic dispatch,
monitor/strobe and deferred assertion readers are currently unproven. Spawning
callee effects also fail closed. A new admitted form requires an explicit
descriptor-retention argument and tests. Expression temporaries and runtime
helper scopes keep their existing heap path.

The runtime exact-pointer index includes both kinds of cell. Scope end destroys
payloads and forgets R1 clock history before unregistering/reusing intrusive
storage; intrusive end with an outstanding retain is a fatal contract violation.
See the [runtime retainer inventory](../rt/scheduler/readme.md#frame-resident-cells).

In a VPI catalog model a subprogram with frame automatics (decision A19-D76)
keeps an activation array of their cell addresses (`_llg_vpi_autos_<n>`, C
stack or coroutine frame), stored where each formal or top-level local is
bound; a real fixed-array local stores its `double` array when declared
(A19-D83). The frame ends (`llg_vpi_frame_exit`) before any scope end or jump
releases the scope holding those locals, so the runtime never reads a
released cell (decision A19-D78); outlined runs hold no such locals.

- Alias visible cells have independent owners, canonical dependency addresses and
  explicit startup/close. A view whose bindings map its whole width onto one
  run of one group slot (`IrSignal::net_view_part`) has no storage, part
  array, descriptor initializer, storage-table entries or start value: it is a
  zeroed `llg_net_alias_t` plus one `llg_net_view_t` row of the model-wide
  `llg_net_views` table, which `llg_net_views_start` configures, binds and
  starts from its group in one loop (`model/storage.rs::render_net_views`);
  a start value written to such a view's storage after the bind never reached
  it, so its `SetScalar` init step is dropped. Other alias parts are run-length rows
  (`model/storage.rs::render_alias_parts` docs); the runtime refreshes and writes each run as one part
  select. A net change rewrites only its own parts of an alias whose parts cover
  its width (`llg_net_alias_ref`), so a vector split into per-bit nets costs one
  part per change; other aliases are rebuilt whole. A ranged publication of a
  wide group shared by many per-bit views rewrites only the parts that read the
  changed bits (`llg_net_alias_span`). Alias/stochastic publication temporaries must be
  registered; alias reads are pure. Sequence-local writes remain private.
- Packed formals resolve through current activation bindings, including callbacks,
  never synthetic model globals. Input mutation is private; ref writes clone/update
  the selected parent and publish immediately. Capture output indices once for
  copy-back; retain const/NBA lifetime rejections. Convert two-state members before
  subsequent missing-index X fill and preserve neighbors.
- Build ref descriptors from typed LHS plans, not legacy address strings. A member
  or packed chain of a flattened array element is an `LLG_REF_VIEW` over the whole
  element, whose selectors bind once with the parent. Register
  LIFO call scopes through copy-out and cancellation. Queue references pin cell
  identities; removed cells detach from queue structure and remain independently
  writable while pinned. The queue's ref list is borrowed, not a second owner.
- `PackedChain` plans contain numeric coordinates only, not owner pointers.
  Evaluate/refine/destroy each index independently and preserve intermediate
  clipping. Invalid unpacked roots supply defaults before inner packed X/no-write
  rules. NBA issue captures masks/values, never an escaping plan pointer.
  `LLG_REF_PACKED_PLAN` is a synchronous borrowed scanner target: do not forward
  it as a retained ref or register it as a pinned queue cell.
- Runtime memory-view selectors are evaluated once into registered owners; validate
  physical view/strides before writes. Fixed folds seed from the first mapped
  value, release per-iteration owners, and bound slots by expression complexity,
  not element count. Include those slots in the explicit frame layout.
- Tagged access checks register expected-tag/equality values, release them before
  the C branch and preserve source diagnostics. A tagged-member NBA also passes
  each guard's issue-time receiver plan to `llg_nba_tagged_selected_after`, so
  the commit re-checks the tags (SV 11.9, Q03). Retag signed native projections
  in both valid and invalid cases. For `inside` array leaves, change the runtime
  value's signedness before wildcard comparison, not metadata alone.
- Writable callback leaves must all be proven private. Never allow external refs,
  net/scheduler writes or NBAs through this path.

## Native boundaries

String inputs are copied; reserve return ownership before argument scopes. Check
cancellation before copy-out. Native ref descriptors borrow addresses, not string
allocations. A native record `ref` formal bound by a specialization
(`IrCallArg::NativeRefBound`) receives null value and companion parameters
that its body never reads. An input event formal is a by-value `llg_event_t` (frame field in a
coroutine callee) holding the object the actual named at the call: the caller
snapshots `{ p ? p->object : NULL }` and the callee binds a private handle copy
(`IrEventRef::Formal`), so rebinding the actual or the formal cannot move waits.
A string, handle, record or packed element or member actual
(`IrCallArg::ElementRef`) acquires an `llg_native_ref_t` at the call, in a value
scope that ends with the call or its cancellation; the formal gets the
reference's stable address and, for a handle, its marker (a packed leaf gets an
`llg_ref_t` over the leaf in that scope), so the callee's code does not depend
on what it is bound to. A record element with queue or dynamic-array members also
gets call-owned companion containers, one per member, each mirrored to the
member's nested slot by `llg_native_ref_mirror` from its own scope object, so
unwinding detaches it before the container goes (`IrElementRef::companions`). A
run-time index into a module-like record picks among the leaves' stable
addresses instead (`IrElementRoot::Leaves`); for a whole record element, a
statement call lowers once per element (`lower_record_ref_select`). A `ref` event
formal bound to a class event property takes the address of its object's slot
(`IrCallArg::EventSlot`), which holds the one `void*` an `llg_event_t` holds; the
same type on both sides keeps this within ISO C aliasing. The callee's frame is
an interior collector root for the object, and a changed slot publishes the
object's handle marker when the call returns.
Generated code holds only the formal's address, so a store through a writable
record `ref` formal tells the element's container itself. `access_address` and
`native_value_address` note a pending `llg_native_ref_value_stored` for every use
of a leaf or the root; reads opt out through `read_lookup`, `native_read` and
`native_value_read`, so a new store form publishes without any change here. The
pending publications go out where the statement ends, and at once after a
`Mutation` or a call, whose enclosing statement may loop or suspend first. A
binding of a member through `ElementRef` publishes through the retained
reference instead, at each store of the callee.
Only open-array DPI imports and tasks waiting on a native record `ref` formal
are templates that are never emitted (`IrFunc::inline_expanded`); a task whose
event control reads a `ref` formal also emits per-signal specializations
(`CallShape`). A typed body waits on a real `ref` formal through its cell, on a
packed one through `llg_ref_wait_spec`/`llg_arm_ref_dependencies` (which expand
the descriptor into the leaf markers of the storage it selects; retained
container cells carry their container's contents marker), on a string `ref`
formal through its `llg_string_t.dependency` and on a handle `ref` formal
through the marker parameter `d<idx>` that follows `r<idx>`
(`handle_marker_param`): the caller passes the module handle's
`<name>_llg_dep`, the object's `llg_class_handle_dependency`, its own `d<idx>`
or a fork-shared handle's `llg_frame_opaque_marker`, else NULL. Stores through
the formal publish that marker at the store (`Frame::handle_store_marker`). A
class string property actual of a waited formal gets its marker before the call
(`observe_class_string_property`); an observed fork-shared string's frame owns
its marker (`IrStmt::SharedString::observed`).
Concrete interface members retain their actual member address.

Container operations evaluate operands in order, register borrowed string keys and
item snapshots, compute all mapped results before publishing, and destroy consumed
intermediates before notifications. Detach consumed keys only at final use. Class
receivers evaluate once before arguments; use typed nominal headers, never casts
between unrelated structures. Class objects are collected (SIM-018): allocate them only
through `llg_gc_alloc`, keep `llg_gc_header_t` first in `llg_class_object_t`,
and make the class type's trace visit every handle, chandle, container and
native-record property. `llg_model_gc_register` (called right after runtime
initialization) registers static handle and container roots, payload tracers
for every container drop type and per-descriptor frame maps. A frame map lists,
per resume state, the `void*` handle fields and object-addressing pointer
fields declared on the live block path before that state
(`FrameLayout::compute_gc_map`); every new resume form must go through
`owned.rs` `resume_state` so its state is recorded. Recursive function
coroutines run synchronously inside a turn and need no frame map. Automatic chandle cells use
the `llg_gc_handle_cell_drop` destructor tag so value scopes trace them.

Mailbox payload capture follows all user operand effects. Remove a consumed node
before callbacks; peek clones. A blocking get into an `AtDelivery` target
(lowering marks whole storage that stays valid for the wait, and class handles) wraps it
in `llg_mailbox_delivery_target`, and the runtime writes it at the put (AB-O3).
A handle destination with a change marker is wrapped in
`llg_mailbox_published_target`, so the runtime publishes a changed handle
whenever it writes it (sync, at the put, `try_get` or on resume).
Any other blocking get emits `llg_mailbox_delivery_take` with the same destination
right after its arm, so a suspended get writes only live storage on resume. Never cast an integer to a target pointer.

DPI-C export entries, thunks and string lifetimes: `model/dpi_exports.rs`
module docs; thunks convert with the import layout helpers and call `<fn>` with
`depth + 1`. When
a model has any context import or export, every import thunk brackets the
call with `llg_dpi_import_enter/leave`; every import call site sets
`llg_dpi_next_site` just before the call (decision S41-D3), and an
uninstrumented thunk clears it after the call. Context imports carry the effects of
every exported body (`execution::effects_for_function`), so cancellation
checks follow them. When an exported task can consume time, a context
imported task is a coroutine shim (ADV-018, `model/dpi.rs`
`render_dpi_context_shim`): its renamed thunk (`<fn>_fcb`) runs on a foreign
call context and returns right after the foreign call when the process was
killed; the waiting export's thunk fills the task's (possibly shared) frame
type on the chain arena and the shim runs it with `LLG_CO_CALL_ARENA`.
DPI input strings remain borrowed for the call. Snapshot the foreign return and
all string outputs before the first copy-out or input destruction, preserving
aliased foreign pointers. VPI argument arrays borrow registered slots only for
one call. File-input/plusarg source and format snapshots plus selected targets
remain owned across callbacks.

## Container storage names

Name containers only through `Frame::container_name`: a model-global
container is its C variable, an activation container is the frame binding made
by `Declare`, a container formal or a call operand (`(*ptr)` into a value-scope
object created by `new_container`), and a class property container is
`llg_class_field(_this, ...)` storage inside that class's methods, or
`llg_class_field(<receiver>, ...)` for a receiver-qualified alias
(`IrContainer::receiver`, a plain handle read rendered without statements).
Fixed-array properties allocate their elements when the object is created.
Activation and per-object containers have no change dependencies, so waits and
monitors on them reject. An `IrDependency::NativeAccess` renders its receiver
with `quiet_receivers` (no null report: `llg_class_field_quiet`,
`llg_vif_member_quiet`) and arms on the field/member storage or, for a handle
property, on the object's lazily allocated `handle_dependency`, which
`llg_class_handle_store` toggles on a changed blocking store (in-place writers
— task output copy-out, mailbox `get`/`peek`, task-form `$cast` — compare
the slot before and after and call `llg_class_handle_published`, as module
handles toggle `<name>_llg_dep`; see `Frame::handle_store_publish`), or, for a string
property, on the marker `llg_class_string_dependency` installs in the string
itself (so every changed `llg_string_move` toggles it; freed with the object
and an interior collector root while armed); a null receiver arms on the
never-changing marker. Calls give every container formal fresh storage and
copy back outputs after the cancellation check, like native values.

## Streaming and event captures

Snapshot a stream RHS before unpacking. Evaluate/check/publish targets in stream
order; later blocking `>>` selectors may read preceding unpacked fields. A `<<`
unpack without a resizable target resolves every fixed selector first, consumes
that many leftmost source bits and reorders only those. Nonblocking streams have
fixed targets only, evaluate selectors at issue and queue each element. Check
available source size before each component's writes, retaining valid in-range
writes on bounds error. Fixed selectors follow declared storage order; dynamic
containers use ascending storage indices. Registered loop indices are destroyed
each iteration. Image-represented fixed targets (refs, locals, members, rows) take
one read-modify-write when blocking and per-element selected stores when queued;
out-of-range source elements stream the element's default-uninitialized value,
and descriptor sources read cells through `peek`. Mixed-domain image elements
convert their two-state runs on the selected segment before any element write.
Descriptor stream operands build a runtime-length source list: empty runtime
selections are skipped, `Selected` windows share owner ranges/cells plus the
element default outside the bounds, `Dense` views and `Packed` values become
scratch cells, and nested streams are lazy images. StreamToFixed left-aligns/
zero-fills and diagnoses oversized inputs. Composite call/ref targets retain exact
layout descriptors.

Evaluate capture values before publishing frames. Synchronous joined forks may
alias registered parent numeric cells until every branch completes. A branch
binds an aliased packed capture under its capture name, its initializer's local
and `IrCapture::alias` (the declaring subroutine's local, which element reads
in nested branches name). Register all
fields of a multi-event context before the first evaluator call: shared context
pointers require one retain per descriptor field, not per distinct pointer.
Fork captures of input event formals use `StorageKind::Event` and a typed
`EventCapture` initializer. Synchronous joins borrow the parent handle through an
opaque slot, preserving rebinding across siblings; the handle stays in the live
parent coroutine frame until all children complete or are cancelled. Detached
joins copy the event object into the slot and bind a private `llg_event_t` in the
branch. Nested forks use the same ownership rule; event objects remain valid
until model teardown.
A `join` branch reaches a packed, real, string, event or handle `ref` formal
of its subroutine through an owned opaque capture whose initializer is
`FormalRead(index)`: the slot holds the formal's descriptor parameter (`rN`),
which the branch rebinds under the same name and type, and the branch frame
takes the subroutine's formal table (`Frame::formal_function`), so formal
reads, stores, selects and waits emit unchanged. The subroutine waits for every
branch, so the descriptor outlives them. A handle formal's second capture, the
same `FormalRead(index)` in the next slot, carries its change marker parameter:
the fork stores the caller's marker (NULL for an actual no other process can
name) in one holder frame slot per formal that every branch aliases, and a
branch binds the marker parameter to that pointer or, when it is NULL, to the
holder's own marker (`llg_frame_opaque_marker`), so sibling branches' stores
and waits meet. A process handle `ref` formal binds the same way; a store
through it (`llg_process_assign` on the caller's counted slot) publishes the
marker when the handle changes, like other handle stores.
A `ref` event formal is its `llg_event_t*` alone unless the model binds a class
event property to a `ref` formal and some wait reads such a property's identity.
Then every writable `ref` event formal also takes `d<idx>`
(`IrFormal::event_marker`, model-wide so overrides and forwarding agree): the
object's `llg_class_handle_dependency` for a property, its own `d<idx>` when
forwarded, else NULL, and `IrStmt::EventAssign` publishes it when the object
changes. The call site also compares the slot at the return, which covers
stores that have no marker parameter (a fork-join branch, an output formal).
