# Emission: evaluation and scopes

Every function that may suspend, every non-final process and every fork branch
uses an explicit POD coroutine frame and the `llg_co_fn` entry ABI. Finals
remain plain `void fn(void)` calls. Route all
typed procedure storage through `Frame::declare` (or its array/loop wrappers):
`CStack` emits the declaration in place, while `CoFrame` first registers a unique
candidate and decides its storage after seeing the complete block tree. A
candidate whose declaring block and descendants have no resume point remains an
ordinary C local; otherwise it is a frame field initialized at the original
declaration point. Coroutine arguments are frame fields. An unshared zero-resume process
keeps marks, temporaries and ordinary procedure storage on the C stack.
Intrusive nodes and cells always occupy frame fields, including zero-resume
processes: a termination check can return before runtime scope unwind. A
frame-less function declares its intrusive nodes/cells in the outer C scope,
so common-return unwinding can access them after an inner block has been left.
`Frame::line` structurally tracks every C body brace and rejects an unbalanced
body. `FrameLayout` records that exact tree, then flattens every chain with only
one storage-bearing child into one struct level. Only two or more storage-bearing
sibling blocks create deterministic `uN.bK` overlays; empty blocks emit nothing.
Under `LLG_CO_DEBUG`, poison each actual sibling-overlay struct on normal exit
and after owner unwind on a nonlocal named-block jump. Loop-break poisoning
must break before rereading the dead condition field; use the debug-only loop
exit macro. Poison completed frame payloads after common-return scope cleanup
to cover early returns. Resolve these sites only
after layout flattening: a flattened child cannot poison a struct containing
still-live parent fields. Lexical scope destruction precedes poisoning; escaped
fork/ref/NBA owners remain in their existing retained heap cells. Callee payload
poisoning precedes argument transfer and declaration initialization for both
embedded call mechanisms and arena acquisition. Preserve headers/anchor prefixes
and the populated-frame CALL macro contract. Emit all added statements inside
`#ifdef LLG_CO_DEBUG`; release emission and frame layouts stay unchanged.
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
offsets use the complete nested member path.

Recursive synchronous subprograms (`ExecutionAnalysis::is_recursive_function`)
render twice from the same body: `<fn>_co` in a `Frame::new_recursive` frame and
the plain-ABI entry `<fn>`, which copies its parameters into an arena frame from
`llg_co_sync_begin` and runs `llg_co_sync_run`. Every caller outside the
component, including dispatchers and DPI export thunks, keeps calling `<fn>`. The
coroutine keeps plain-function exits (`llg_rt_exiting`, budget and
cancellation `goto _llg_return`, `llg_current()` for `self`), numbers its
resume points as it emits arena calls, and writes its result through the
`_llg_result` frame pointer at `_llg_return`; the guard writes the result
default. Arena call sites store by-value packed descriptors, `depth + 1` and a
destination that survives the resume (temporary slot, reserved native object or
frame scalar). Dynamic calls use per-slot/method helpers returning an anchor for
a recursive implementation and NULL after a plain call or dispatch failure.
Class virtual slots with a suspending implementation
(`ExecutionAnalysis::suspendable_dispatch_slots`, SIM-011) use the same
helper from any coroutine at an arena call site; their plain dispatchers
fail instead of entering a coroutine with plain arguments.
A failed null class or virtual-interface access is fatal at once (S99-D8; a
virtual-interface failure skips the final procedures, a class one runs them,
S99-D11):
inside a statement, the owned emitter checks the access result and leaves the
process (`LLG_CO_EXIT_CHECK`, or `goto _llg_return` in plain functions) before
the rest of the statement, and checks the exit flag after a plain or dispatched
call whose effects include `Terminate`. The checks sit on the failure branch, so
code without such accesses or calls is unchanged.
The legacy fragment emitter (`statements.rs`, `expressions.rs`, `objects.rs`
and their children, and the `#[allow(dead_code)]` legacy model renderer in
`model/`) does not get these checks and is development-only: its public entry
points `render_expr`, `render_stmt` and `render_pre_fn` return
`OWNER_MIGRATION_DIAGNOSTIC` before rendering, no code outside `emit_c` calls
them (`emit_decoupling::legacy_fragment_emitter_has_no_callers_outside_emit_c`,
`legacy_fragment_emitter_rejects_valid_ir`), and with those entry points cut off
rustc reports `render_stmt_impl` and `render_expr_impl` unused. Port a
behaviour to it only together with a user-reachable path.
Compute conservative LP64 upper
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

Cached C-local mirrors of frame fields: `owned/cached_fields.rs` module docs
(`_llg_temp_scope` is write-only and not a candidate). A candidate that narrowing
turned into a C local, or with a member access, also stays a frame field. Keep reload placeholders out of
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

`Value` carries code, width/sign/fill metadata and an optional owning descriptor slot.
`expression` returns an owner; `operand` may return a synchronous packed borrow
with its original registered address for later ownership materialization.
Emit ordered setup, calls and cleanup, not nested allocating C expressions.
Non-addressable real results are scalar temporaries; addressable real locals use registered stable cells selected by the escape
proof below.
Packed locals have separate lexical cells from expression temporaries.
No compiler cleanup attributes, statement expressions, VLAs, alloca, C++
destructors or simulation-lifetime temporary arena.

Borrow packed inputs, own every value-returning operation, and destroy consumed
owners immediately. `owned/operands.rs` admits direct packed signal/local and
pooled-constant borrows only at synchronous consumption sites. A binary's earlier
operand is snapshotted when the later subtree can call, mutate, suspend or contains
an unproven kind. Selected assignment targets keep an owned RHS while capturing
selectors; whole signal/reference assignments may borrow through their one
alias-safe publication call. General expression, call arguments, delayed writes,
stream fan-out, sampled reads and callback outputs retain owners. Never destroy,
move or retag a borrow. Read-only private stores materialize an owner before move.

Elide packed cast/resize only when width and signedness match and there is no
context fill. Two-state coercion remains explicit. VPI callback results first
normalize to their declared HDL width/sign: registration
may choose a different runtime shape, and the call arguments specify fallbacks.
Reuse an owned operand's descriptor for a fresh result, choosing the later owner
when the earlier operand is borrowed. Install every packed or string result
through `emit_c::destinations` (why and how: its module docs); `move_string` too.
`tests/sim_stack_bounds.rs` guards frame growth. Register a new value-returning
runtime call there together with its `_to` declaration. Non-coroutine functions return packed results through a trailing
`sv4_t* _llg_result` and borrow packed inputs as `const sv4_t*`. Wide constant
limbs used by startup code live in file-scope tables, not compound literals.
Runtime arithmetic may reuse same-width payloads; multiply must preserve aliased
inputs until completion. No HDL destination is mutated before scheduler publication.

Each model's `PackedConstants` registry deduplicates canonical packed constructors
by width, signedness and all three planes, including expanded fills. Procedures
borrow its immutable owners or clone when a retained result is required. Initialize
constants before model defaults/initializers and destroy them after runtime queues
and model storage at close. One-limb literals use the initializer directly,
keeping them cheap with inline small values and avoiding extra startup owners.
Periodic wide constants: `PERIODIC_CONSTANT_MIN_BITS` and `periodic_constant`
docs in `constants.rs`. Registration follows deterministic typed emission, never generated
C parsing or corpus-specific rules. Ending a local scope drops its lexical cell
reference; pending NBA/clocking records retain the descriptor until commit/discard.
This protects identity, not shared/COW packed values. Preserve procedure-root
arguments/results across yields. Clone returns before unwinding cells that queued
writes may still reference. Cancellation unwinds registered scopes without
resuming coroutines or depending on C stack unwinding.

Register pooled constant width/sign facts with instance sharing. Generated
instance bodies may load immutable constants from typed record operands, just as
their literals previously varied per site; constant pointer identity does not
constrain the sharing key. Preserve the existing width/sign and non-generated
parameter-payload exclusions.

Freeze output targets/indices before calls. Keep arguments/copy-out temporaries
alive across suspension; do not admit native-real NBA targets without equivalent
retention. Inline numeric calls reserve escaping results in the caller before
private callee storage and rename internal labels. Static callback returns require
the lowering proof of independence from prior calls; ordinary static calls retain
persistent return storage. Static output temporaries retain prior values unless
inout initialization supplies a value; automatic ones receive typed defaults.

`UdpEval` references one typed model-level definition. `owned/udp.rs` emits one
immutable table per definition, shared by scalar and array instances (dense index
and row-scan split: its module docs). The dense index is built from the rows at
generation (first match wins, unmatched X); an evaluation reads each input's
bit-0 state with `llg_sv4_state` and makes one `sv4_fill` of the loaded state.
Inputs are read in order; they are borrowed in place only when every input is a
stable expression, and a constant bit or one-bit part-select of a borrowed base
reads that bit directly, so no input owner or copy exists. Wider definitions use
one `sv4_udp_eval` scan. Gate lowering passes plain
scalar input expressions and keeps selected output indices, sensitivity,
strengths and inertial publication. Row count affects only table data, never the
executable gate body.

Module path sets are table data, not per-instance initializers
(`owned/model/initialization/instance_tables.rs`). Storage emits the zeroed
set records and pools: `llg_path_dest_pool`, the writable row pool
`llg_path_row_pool` (one `llg_module_path_t` per path, set-major, so SDF
entries address `&llg_path_row_pool[row]`) and, when limits are used,
`llg_path_limit_pool`. One interned `llg_path_template_<k>_*` row/destination/
style/limit table per distinct set shape (a module's specify block) plus one
`llg_path_templates[]` row carry everything instance-independent, including
the condition evaluator; per set, `llg_path_sets[]` holds `{ set, scope,
template, condition context }` and the instance's source and destination
references fill `llg_path_source_refs` and `llg_path_dest_nets`. The storage
defaults run one `llg_module_paths_layout` loop, which copies template rows
into the pools in set order, before the pulse configuration, startup SDF and
the `llg_module_paths_start` loop. ADV-008 condition evaluators
(`llg_path_set_<n>__conditions`, MonEval-shaped) take the set's
`condition_context`: an evaluator shared by instance sharing reads its
instance record from it, an unshared one gets NULL.
`llg_module_paths_configure` is emitted only when `IrModel::path_pulse`
differs from its default. Models without paths are unchanged.
Switch networks emit two constant tables after the net groups
(`llg_switch_nodes`: net pointer and bit; `llg_switches`: node pair and
flags), an optional direct-control table (`llg_switch_controls`: control
storage, bit, switch, invert) and one `llg_switches_start` (then
`llg_switch_controls_start`) after the net delays start; a `SwitchControl`
statement is one `llg_switch_control` call. Models without
switches emit neither, so their `model.c` is unchanged.
Timing check sets follow the same template scheme: storage emits the set
records and the terminal, check and line pools (`llg_timing_check_pool` rows
are writable for annotation and addressed by SDF as
`&llg_timing_check_pool[first + check]`) and `llg_timing_set_<n>__pairs` for
combined checks a negative annotation re-solves; `llg_timing_template_<k>_*`
terminal/check/line tables and `llg_timing_templates[]` rows hold each set
shape once, `llg_timing_values` (terminals, then notifiers) and
`llg_timing_line_nets` hold the instance references, and one
`llg_timing_checks_layout` loop runs before `llg_timing_checks_configure(0)`
(when reports are off), the doorbell and the `llg_timing_checks_start` loop.
Rows carry `condition2`, `flags`, notifier indices and the set its `.lines`
(delayed signals: source/target terminal, net and driver slot or none,
rise/fall). `TimingCheckStep` emits `llg_timing_checks_run();`. Models
without timing checks are unchanged byte for byte.
SDF annotations (`model/storage.rs` `render_sdf_annotations`) emit, only
when the model has any: the writable gate delay array
`llg_sdf_gate_delays` (annotated gates read their triple from it),
`llg_sdf_load_<n>` interconnect entry tables, and one
`llg_sdf_annotation_<n>` per annotation (a `llg_sdf_op_t` array with
designated `.kind/.flags/.delay/.reject/.error/.derive/.first` and target
fields, and the value array). `IrStmt::SdfAnnotate` emits
`llg_sdf_apply(&llg_sdf_annotation_<n>);`; startup annotations emit the same
call in the storage defaults, after the path and timing layouts and before
the first `llg_module_paths_start`.
Only destination groups gain `.propagation_enabled = 1` and per-bit
path-stage records; a model without paths emits exactly what it did
before. A delayed group with separately driven parts emits its part bounds
and one record slot per part; a group that starts X calls
`llg_net_start_unknown` in the storage defaults, after the runtime delay
store when its net delay is runtime-valued. Groups with `driven_runs` (T0-D3)
share deduplicated run tables and call `llg_net_start_driven` after the net
storage loop; the storage teardown calls `llg_net_start_release`. Startup
net fills (`WriteNet`) use `llg_net_write_driven`, which settles only the
bits the value drives. `ModulePathStamp` is one `llg_module_path_stamp` call; the legacy
statement renderer rejects it.

A sequential definition emits one `static inline llg_udp_step_<t>` and each
instance one file-scope state record (`IrModel::udp_states`) holding the
base-3 code of its prior inputs and its state, initialized to all-X inputs
and the initial value. `UdpStep` advances the record from the current input
digits; `UdpState` reads its state with one `sv4_fill`. While the next-state
index `3^(n+1) * (1 + 2e)` (`e` inputs named with a transition) fits
`UDP_SEQUENTIAL_DENSE_MAX_ENTRIES` (2^20 bytes) the step makes one index load
per changed input: slot 0 serves inputs without transitions, and each edge
input has a slot per `old - (old > new)`. The index is built by evaluating
`IrUdpSequential::next_state` for every entry, so the emitted lookup and the
IR rule cannot diverge. Larger definitions keep encoded rows and scan them
in the step (level rows, then edge rows on the changed input). Changed inputs
are taken in port order (decision S3-D2) and Z/X changes are skipped.

ArrayMux uses uniform leaf defaults; StructMux passes one period's leaf layout to
`sv4_record_conditional_merge`.
Evaluate each branch once and preserve its self-determined shape; an enclosing cast
must not resize operands before the merge. Predicates evaluate only reached clauses,
convert packed/real truth to one bit, and allocate nothing for skipped clauses.
Pattern member projections convert two-state fields before comparison or binding;
dispose the raw slice when replacing it with the converted value.
Ambiguous muxes evaluate both arms before merging (real results become zero).
Do not emit detached predicates or bypass eligibility through wrappers.

`owned/repeated_values.rs` owns concatenation append emission (loop thresholds:
its constants' docs); the first operand still seeds
the result outside the loop. Evaluate, append and
destroy each operand inside the loop, preserving changing reads and mutations.
The accumulator's runtime width grows each iteration; its Rust metadata records
the final checked width. Only explicitly admitted numeric kinds whose temporary
slots are released locally qualify. Calls and other kinds with distinct resume
sites or lexical storage remain expanded. Declare loop storage through `Frame`
and emit all braces through `Frame::line` so scope narrowing and C9 reloads hold.

`owned/dense_cells.rs` (purpose: module docs) applies to runs of at least
`LLG_DENSE_CELL_RUN_MIN` cells of one packed fixed array (not real, nor a net
for writes); descriptor storage means sparse, activation or descriptor-flagged
arrays, whose reads peek without materializing cells. Reads: concatenation operands, with nested
concatenations spliced in, that are constant-index whole-cell `ArrayRead`s of
consecutive storage offsets become one `*_image` call (not under sampled
reads). Writes: consecutive `Assign`s in one statement sequence to
consecutive whole cells, with one `nba` flag, whose right sides are one constant
(`*_fill`) or same-width slices of one captured local stepping down by the cell
width, under same-width sign conversions or `ToTwoState` (`*_store`); and a
left-to-right streaming target, as an assignment or call copy-out, whose leaves
are exactly such a run. The runtime writes cells in order through
`llg_ba`/`llg_nba_after` after the same cast and two-state conversion as
`store`, so publication, force and NBA behaviour are per cell as before.
Shorter runs and any other shape keep per-element code.
