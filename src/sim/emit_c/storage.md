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

- Alias visible cells have independent owners, canonical dependency addresses and
  explicit startup/close. Alias parts are run-length rows
  (`model/storage.rs::render_alias_parts` docs); the runtime refreshes and writes each run as one part
  select. Alias/stochastic publication temporaries must be
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
