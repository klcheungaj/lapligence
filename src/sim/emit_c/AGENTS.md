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
`CStack` emits the declaration in place, while `CoFrame` registers a unique field
and emits only its initialization at that point. Arguments are frame fields too.
`Frame::line` structurally tracks every C body brace and rejects an unbalanced
body. `FrameLayout` records that exact tree, then flattens every chain with only
one storage-bearing child into one struct level. Only two or more storage-bearing
sibling blocks create deterministic `uN.bK` overlays; empty blocks emit nothing.
Parent storage remains live and typed declarations still bind to the active raw
block before finalized paths rewrite the generated body. Do not emit a
coroutine-body structural brace outside that path or retain an overlaid field
access after its block closes.

Emit coroutine frame types callee-first. Each storage-bearing block's polled
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

Within each struct level, fields observed by the first generated continuation
statement after a Phase-2 suspension are emitted first, preserving declaration
order within hot and ordinary groups. The continuation probe ends at the next C
statement boundary, including the cancellation check after a wait. This is a
deterministic cache-line heuristic, not a liveness proof.

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
allocations. Inline event formals retain event identity, not numeric encoding;
concrete interface members retain their actual member address.

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

Maintain exact scope/reference checks, array-index disposal, inert invalid handles
and source-size preflight regressions. Preserve these distinct validation layers:
Rust structure checks, real IR-to-C emitter execution, public HDL CLI runs and
handwritten C components. Hand-authored output-shape probes do not execute this
emitter. Use [ownership validation](../../../tests/readme.md#dynamic-ownership-validation)
and its generated-C sanitizer configuration; do not infer feature acceptance from
component tests.
