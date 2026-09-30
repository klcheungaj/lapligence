# Owned C11 emission

Applies to `emit_c.rs`, `emit_c/owned.rs` and children. Read
[pipeline](../AGENTS.md), [lowering](../codegen/AGENTS.md) and
[runtime ownership](../rt/AGENTS.md). Consume validated `ExecutionModel` only;
no frontend/DB/FFI/VPI dependencies or reparsing opaque C fragments.

## Evaluation and scopes

Every function that may suspend, every non-final process and every fork branch
uses an explicit POD coroutine frame and the `llg_co_fn` entry ABI. Finals
remain plain `void fn(void)` calls. Route all
typed procedure storage through `Frame::declare` (or its array/loop wrappers):
`CStack` emits the declaration in place, while `CoFrame` first registers a unique
candidate and decides its storage after seeing the complete block tree. A
candidate whose declaring block and descendants have no resume point remains an
ordinary C local; otherwise it is a frame field initialized at the original
declaration point. Coroutine arguments are frame fields. An unshared zero-resume process
therefore has only the `llg_co_frame_t` header and keeps its value scope, marks,
temporaries and procedure storage on the C stack.
`Frame::line` structurally tracks every C body brace and rejects an unbalanced
body. `FrameLayout` records that exact tree, then flattens every chain with only
one storage-bearing child into one struct level. Only two or more storage-bearing
sibling blocks create deterministic `uN.bK` overlays; empty blocks emit nothing.
Parent storage remains live and typed declarations still bind to the active raw
block before finalized paths rewrite the generated body. Do not emit a
coroutine-body structural brace outside that path or retain an overlaid field
access after its block closes.

Scope narrowing is safe because resume dispatch labels occur only in blocks
marked as containing their suspension site: dispatch never jumps into a narrowed
block or past a narrowed declaration's initialization. Cancellation, loop-control
and named-block gotos only leave lexical blocks. Keep GCC
`-Werror=jump-misses-init` as an independent check. The generated-C escape lint
may accept the address of a local declared in a resume-free block, but must reject
an address of C-stack storage whose declaring block contains a resume point.

Emit coroutine frame types callee-first, one typedef per distinct layout:
frames that render identically once embedded callee types are replaced by their
own layout group (every instance of one process or task) share one
`llg_shared_frame_<k>_t`, while a layout used once keeps `<fn>_frame_t`; bodies,
descriptors and layout assertions all name the shared type. Each storage-bearing block's polled
callees occupy ordinary members of its deterministic `union callsN`, anchored
callees use `LLG_CO_ANCHORED(T)`,
and recursive or oversized callees use `ch->arena`. Descriptor
offsets use the complete nested member path. Compute conservative LP64 upper
bounds with every embedded call charged its 16-byte anchor prefix and sibling
blocks contributing their maximum rather than their sum; the named
`ExecutionAnalysisOptions::embed_limit` tunable defaults to 16 KiB and forces
larger callees onto the arena. Descriptors contain the real entry function;
numbered site tables and dispatch cases must remain one-to-one with every
emitted await or suspendable call.

Specification arrays passed to a runtime arm that copies them before returning
(`llg_arm_any_dependencies`, `_any_events`, `_events`, `_order`, `_mixed`,
`_clocking_cycle`, `_expressions` and the dependency lists inside its
descriptors; the copy contract is documented on each declaration in
`llg_rt.h`) are compound literals inside the `LLG_CO_AWAIT` arm expression via
`Frame::arm_array`: no frame field, no `memcpy`. A literal's lifetime is the
enclosing block, which covers the arm call. Route only arm arguments through
it, and only when the runtime does not keep the array or a pointer into it;
nonblocking registrations (`llg_nba_event_*_when`), `wait_order`'s result flag
and event handles the runtime resolves later keep declared storage.
Initializers are evaluated into scalars before the await, so a literal may be
repeated at several arm sites (clocking cycles).

Within each struct level, fields observed by the first generated continuation
statement after a Phase-2 suspension are emitted first, preserving declaration
order within hot and ordinary groups. The continuation probe ends at the next C
statement boundary, including the cancellation check after a wait. This is a
deterministic cache-line heuristic, not a liveness proof.

Frame fields that are assigned once and only read afterwards (`_llg_t`,
`_llg_frame_base` and the `_llg_local_N` cell pointers; `_llg_temp_scope` is
write-only) are mirrored by C locals of the same name (`owned/cached_fields.rs`,
design §13.2 rule 2), because every `F->` load is possibly aliased for GCC. The
local is declared without an initializer before `LLG_CO_DISPATCH_BEGIN`, assigned
together with the field (`x = F->x = init;`, or right after the prologue) and
reloaded from the frame on the line after every suspension macro, for the cached
fields whose C scope is open there: a resume jumps into the macro with every local
indeterminate. Stability rests on the runtime never moving a value scope's array
and on the prologue being skipped by the dispatch. Emission still checks the final
text: a candidate with any other write, address-of or member access, one that
narrowing turned into a C local, and one read no more often than it is reloaded
(`CACHE_MIN_READS_PER_RELOAD`) stays a frame field. Keep reload placeholders out of
`Frame::line`, which would make them the continuation probe's first statement. Add
a new cached kind only after proving it is assigned only at its declaration. The
frame lint rejects a cached local read after a suspension without a reload.

Cancellation checks (`llg_activation_cancelled()`) follow only cancellation
points: resume points, `disable`, calls whose callee may disable (the Phase-2
`Disable` effect) or dispatch dynamically, and the exit of a named block that
was left through a check. Only `llg_disable_target` sets an activation's
`disabled` flag, so nothing else can cancel a running process. A check straight
after a call, an await or a leaf statement covers its points; compound
statements re-check at their end while any point inside is uncovered, so a
point in one branch is never checked only in another. Copy-out and later
operands after a cancelled call stay skipped.

`Value` carries code, width/sign/fill metadata and an owning descriptor slot.
Emit ordered setup, calls and cleanup, not nested allocating C expressions.
Non-addressable real results are scalar temporaries; addressable real locals use
registered heap-backed doubles, never a C stack address that is invalid after
a stackless return.
Packed locals have separate lexical cells from expression temporaries.
No compiler cleanup attributes, statement expressions, VLAs, alloca, C++
destructors or simulation-lifetime temporary arena.

Borrow packed inputs, own every return, destroy consumed operands immediately and
reuse only empty descriptor slots. Ending a local scope drops its lexical cell
reference; pending NBA/clocking records retain the descriptor until commit/discard.
This protects identity, not shared/COW packed values. Preserve procedure-root
arguments/results across yields. Clone returns before unwinding cells that queued
writes may still reference. Cancellation unwinds registered scopes without
resuming coroutines or depending on C stack unwinding.

Freeze output targets/indices before calls. Keep arguments/copy-out temporaries
alive across suspension; do not admit native-real NBA targets without equivalent
retention. Inline numeric calls reserve escaping results in the caller before
private callee storage and rename internal labels. Static callback returns require
the lowering proof of independence from prior calls; ordinary static calls retain
persistent return storage. Static output temporaries retain prior values unless
inout initialization supplies a value; automatic ones receive typed defaults.

ArrayMux uses immediate-element defaults; StructMux uses immediate-member plans.
Evaluate each branch once and preserve its self-determined shape; an enclosing cast
must not resize operands before the merge. Predicates evaluate only reached clauses,
convert packed/real truth to one bit, and allocate nothing for skipped clauses.
Pattern member projections convert two-state fields before comparison or binding;
dispose the raw slice when replacing it with the converted value.
Ambiguous muxes evaluate both arms before merging (real results become zero).
Do not emit detached predicates or bypass eligibility through wrappers.

## Storage, references and publication

- Alias visible cells have independent owners, canonical dependency addresses and
  explicit startup/close. Alias/stochastic publication temporaries must be
  registered; alias reads are pure. Sequence-local writes remain private.
- Packed formals resolve through current activation bindings, including callbacks,
  never synthetic model globals. Input mutation is private; ref writes clone/update
  the selected parent and publish immediately. Capture output indices once for
  copy-back; retain const/NBA lifetime rejections. Convert two-state members before
  subsequent missing-index X fill and preserve neighbors.
- Build ref descriptors from typed LHS plans, not legacy address strings. Register
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
  the C branch and preserve source diagnostics. Retag signed native projections
  in both valid and invalid cases. For `inside` array leaves, change the runtime
  value's signedness before wildcard comparison, not metadata alone.
- Writable callback leaves must all be proven private. Never allow external refs,
  net/scheduler writes or NBAs through this path.

## Native boundaries

String inputs are copied; reserve return ownership before argument scopes. Check
cancellation before copy-out. Native ref descriptors borrow addresses, not string
allocations. An input event formal is a by-value `llg_event_t` (frame field in a
coroutine callee) holding the object the actual named at the call: the caller
snapshots `{ p ? p->object : NULL }` and the callee binds a private handle copy
(`IrEventRef::Formal`), so rebinding the actual or the formal cannot move waits.
Output, inout and ref event formals, event controls reading by-value formals or
locals and class/virtual-interface event calls stay inline-expanded templates that
are never emitted; a task whose event control reads a `ref` formal is a template
whose per-signal specializations are emitted (`subroutine_requires_inline`,
`CallShape`). Concrete interface members retain their
actual member address.

Container operations evaluate operands in order, register borrowed string keys and
item snapshots, compute all mapped results before publishing, and destroy consumed
intermediates before notifications. Detach consumed keys only at final use. Class
receivers evaluate once before arguments; use typed nominal headers, never casts
between unrelated structures. Model registries retain cyclic objects until close;
this is not garbage collection.

Mailbox payload capture follows all user operand effects. Remove a consumed node
before callbacks; peek clones. Delivery snapshots and automatic destinations remain
registered/pinned through reentrant publication and cancellation. Never cast an
integer to a target pointer.

DPI input strings remain borrowed for the call. Snapshot the foreign return and
all string outputs before the first copy-out or input destruction, preserving
aliased foreign pointers. VPI argument arrays borrow registered slots only for
one call. File-input/plusarg source and format snapshots plus selected targets
remain owned across callbacks.

## Streaming and event captures

Snapshot a stream RHS before unpacking. Evaluate/check/publish targets in stream
order; later selectors may read preceding unpacked fields. Check available source
size before each component's writes, retaining valid in-range writes on bounds
error. Fixed selectors follow declared storage order; dynamic containers use
ascending storage indices. Registered loop indices are destroyed each iteration.
StreamToFixed left-aligns/zero-fills and diagnoses oversized inputs. Composite
call/ref targets retain exact layout descriptors.

Evaluate capture values before publishing frames. Synchronous joined forks may
alias registered parent numeric cells until every branch completes. Register all
fields of a multi-event context before the first evaluator call: shared context
pointers require one retain per descriptor field, not per distinct pointer.
Fork captures of input event formals use `StorageKind::Event` and a typed
`EventCapture` initializer. Synchronous joins borrow the parent handle through an
opaque slot, preserving rebinding across siblings; the handle stays in the live
parent coroutine frame until all children complete or are cancelled. Detached
joins copy the event object into the slot and bind a private `llg_event_t` in the
branch. Nested forks use the same ownership rule; event objects remain valid
until model teardown.

## Model lifecycle and checks

Initialization frames allow legal zero-time declaration calls, not timing-bearing
functions. Tear down runtime queues and VPI observers before model storage.
`llg_model_start`/advance/close preserve suspended state; advance returns status 2
without cleaning up that live context. Reject double start, allow repeated
start/close, and preserve `LLG_MODEL_NO_MAIN` for host-controlled entry. The process-global runtime
supports one model, not concurrent/thread-safe instances. Keep ABI 4/cache markers
aligned and stale generated C rejected. Emit `LLG_MODEL_PROCESS_ABI 2`, pass
only immutable descriptors to spawn/fork sites, and initialize through
`llg_rt_init_with_args_and_precision`; coroutine stack sizing is not model data.
Default and teardown calls for plain static storage use one file-scope pointer
table and loop per value representation, flushed at the storage section's
original position. Each net's contribution cells occupy one exact-size `__cells`
array; the unchanged driver pointer table points into it. Net defaults, driver
reset/destruction, index reset and alias-list cleanup use a single immutable
net/fill descriptor table and fixed-size loops, independent of net count, driver
count, width and resolution kind. Strength/index/scratch storage remains
exact-size; only mutable index state needs resetting. Scalar aliases copy all
visible owners before binding; array aliases copy and bind in original array/
element order through one pointer table. Array initialization refreshes only its
contiguous slice of that table. Runtime queues and VPI must close before these
loops destroy values and clear alias lists; repeated start/close rebuilds them.
Runs of consecutive constant startup calls (spawns, finals,
waveform registrations) use argument tables and loops in their original order;
registrations still fail startup. Generated identifiers are standard C (never
`$`, checked by the all-fixture lint); synthesized procedural-assign enables use
reserved `llg_pca_en_<n>` names.

`names.rs` owns reversible `cI_` / `Z` source-name escapes and injective
component composition. Lowering retains raw source components separately from
display paths: never pass encoded components through `ident` again. Keep the
source diagnostic labels, VPI and waveform metadata, design/export
file names and foreign DPI C symbols independent of internal symbol spelling.
After frame sharing and all derived symbols are emitted, cap complete internal
identifiers at `MAX_C_IDENTIFIER_LEN` (128) through a sorted per-model registry.
Skip existing symbols when assigning short indices; preserve namespace and
frame/descriptor suffixes. The existing emitted-C identifier rewrite passes
skip literals/comments/numeric suffixes; this does not admit opaque IR fragments.
Explicit foreign DPI names are exempt from the internal bound. Coroutine descriptor
labels and loop budgets use source process/function labels; branch descriptors use
the source owner plus `.fork`, never the helper C symbol. IR functions without source
provenance use an unnamed-function diagnostic label.

Maintain exact scope/reference checks, array-index disposal, inert invalid handles
and source-size preflight regressions. Preserve these distinct validation layers:
Rust structure checks, real IR-to-C emitter execution, public HDL CLI runs and
handwritten C components. Hand-authored output-shape probes do not execute this
emitter. Use [ownership validation](../../../tests/readme.md#dynamic-ownership-validation)
and its generated-C sanitizer configuration; do not infer feature acceptance from
component tests.

## Procedural continuous assignment batches

Process statement lists batch only adjacent `PcaAssign` operations with exactly
equal source representation, selection/conversion IR and target type. The typed
source whitelist permits signal/static-local reads, part selects, constant
bit/indexed selects and numeric conversions; the execution effect summary must
contain only `ImmediateStore`. Dynamic selectors, computed/calling sources,
automatic cells, sampled reads and resolver-based net aliases remain unbatched.
Every other statement, lexical boundary and execution-block boundary breaks a
run. `PCA_BATCH_MIN_ASSIGNMENTS` is the named minimum (4); shorter runs retain
their original emission.

Each batch has a file-scope `static const` source/target/enable/binding table.
One model-local non-inlined helper per exact typed shape uses the ordinary owned
expression/conversion emitter and the caller's registered temporary slots. Each
row evaluates/clones its source, applies every original selection/cast, publishes
with its own binding and destroys its owners before the next row. Repeated
targets and reads of earlier targets remain legal; never hoist source values.
The loop has a separate resume-free block, and its index uses `Frame::declare`.
Deassign, drive, force/release and callable/final bodies retain their original
paths. Tables are typed instance-record operands, so shared processes load their
own rows; resolved-net sources still address C14 storage normally. No runtime or
coroutine ABI changes are involved.

## Instance body sharing

`model/sharing.rs` groups compiler-owned emitted bodies by source location,
coroutine/ordinary ABI, exact canonical frame layout and typed operand shapes,
then requires exact equality after normalization. PCA drivers also share across
distinct source sites when their typed driver operations and normalized bodies
match; each original source location remains in its descriptor. Unrepresented
references remain in the key. Widths, resume numbers, array extents and ordinary
parameter payloads remain structural; generate-instance ULL payloads may become
record fields.
Only known activation/PCA/net-contribution identities become scalar operands.
Bodies with local static storage stay separate. Startup-external roots (assertion
actions) stay separate because they do not receive the startup record initialization.

`LLG_SHARE_MIN_INSTANCES` is a positive integer (default 4); `unlimited` disables
sharing. Each qualifying class emits one body and static const typed records.
Operands equal across all members stay direct constants/references. Shared root
frames add `_llg_instance` immediately after the header; spawn tables supply its
`offsetof` and initialize it before execution, including Reactive program roots.
Original descriptors, sites, runtime labels and all model storage remain distinct.
Metadata offsets use the resulting frame type. Shared callees/fork entries keep
small ABI-preserving thunks forwarding a hidden record argument; their frame
layout and arena/poll analysis therefore do not change. Shared bodies resist
compiler inlining/cloning to avoid recreating the duplicated code.

The immutable record pointer is reconstructed at every entry, before dispatch;
it is not a C9 local whose initial assignment can be skipped on resume. Records
hold model-lifetime storage/descriptor addresses, never activation locals. The
frame lint must recognize shared-body signatures and narrowly permit addresses of
record-referenced model storage while retaining its cached-field and overlay checks.
