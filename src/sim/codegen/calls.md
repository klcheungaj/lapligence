# Lowering: calls, loops and callbacks

Capture each input once before dependent defaults. Preserve exact fixed activation
payloads; automatic locals/returns are independent, static formals/locals/returns
persist definition-wide. Input/inout copy in per call, output/inout copy out at
return as an assignment whose selectors, keys and handles are evaluated then
(decision S13-D1, `TargetSelectors::AtCopyOut`; the emitter resolves the target
after the callee returns). A whole class property, column-layout record
destinations and mailbox `ref` destinations still bind before invocation. Explicit local lifetimes
apply individually (static-in-automatic and automatic-in-static). Static output
formals are not implicitly reset/copied in. Static return storage is ordinary
hidden model storage, not an opportunistic expression substitute.

Ref/const-ref forward original leaves through selected views/nested calls; never
invent globals for automatic formals. Task inlining and timed-recursion
rejections: `lowering.rs` module docs. NBA targets must outlive
publication: fixed static subroutine arrays use persistent per-instance signal
storage and typed selections; automatic/loop-local and unsupported native
subprogram storage reject.
String/chandle/native paths need typed owners/captures; chandles stay `void *`,
never integers. Preserve explicit C names and pure/context qualifiers in canonical
`svdpi.h` scalar thunks; reject missing/conflicting libraries/signatures before
simulation and retain packed/open-array/ref guards. Exports (`IrFunc::dpi_export`)
keep the import formal/result rules, reject time-consuming tasks (ADV-018),
reserved or colliding C identifiers and duplicate (scope, C identifier) pairs;
`finish_dpi` sorts `IrModel::dpi_scopes` by name (the runtime binary-searches
them) and import call sites carry `dpi_site` for `svGetCallerInfo` (S41-D3).
Static export formal storage: `IrDpiExport::storage` docs.

Named activations (`activation_target`) pair the declaration with its module
instance or package; inside a class method, which has neither, the class
specialization is the environment, so a `disable` ends the block or task in
every object (CD-D1). Class task bodies and call sites carry the task's own
activation scope only when some `disable` names the task
(`task_is_disable_target`); other class tasks keep none.

Ordinary forks use owned activation frames. Completion/cancellation releases
captures; synchronous join may borrow live parent storage, detached joins snapshot
admitted values. Keep blocking-function-join and unsupported capture/formal guards.
Loop variables have unique lexical identities; foreach retains omitted dimensions,
declared traversal order and signed endpoints. Break/continue target the innermost
source loop. Function steps preserve list order, discarded-result ownership and
copy-out; continue executes the step, break/return do not. Reject task/method/system
steps not admitted by the function-step path.

Numeric expression callbacks are automatic and effect-free, or static with a proof
that every normal return ignores previous state. Analyze abrupt exits separately;
loops consume their own break/continue, unmodeled flow fails closed. Private
assignment-bearing arithmetic/stream leaves are allowed only after proving every
target private; reject external refs, scheduler/net writes, timing, NBAs, static
locals or unproved static returns. `EmptyArgument` wrappers do not bypass checks.
`classify_event_expression` separates those callback rejections from legal
zero-time helpers that only fail read-only proof (visible or persistent writes,
unproved static results, descriptor-array formals/results): a blocking event
control with such a helper becomes a process-evaluated loop (evaluate at arm,
`WaitAny` on the union of read sets, re-evaluate every source, LSB edge tests,
qualifier on detection; declared named events by trigger count, reals by IEEE
bits). Its sensitivity (`collect_evaluator_sensitivity`) excludes storage
declared in or written by the helpers (SV 9.2.2.2.1), so waits sharing a static
helper cannot wake each other forever. `ProcessEventPlan` splits arming from
waiting: intra-assignment NBA and `->>` controls arm at issue and wait in a
`IrJoinKind::Detached` captured branch. Effectful force sources evaluate into a
hidden per-site source signal, re-evaluated by a guard process gated by
`IrRuntimeQuery::ForceSourceActive`. `$monitor`/`$strobe` helpers
(`classify_postponed_expression`) may store only to storage they own
(`DisplayEval.private_effects`); other writes reject (SV 4.4.2.9). Callee
activation arrays never enter dependency sets. Writes inside functions called by a continuous
assignment are procedural writers in the SV 6.5 conflict check; only its LHS
is the continuous driver.

Fixed reductions map immediate elements rather than recursively flattening rows;
peel one unpacked dimension at a time. Preserve named/default iterator identity,
signed 32-bit declared indices, enclosing reads and restored bindings even on
failure. Reject iterator writes, illegal dimensions, unmapped nonintegral rows or
nonintegral maps. Seed with the first mapped element to preserve singleton Z.
Reverse freezes receiver coordinates once; sort captures every element and key
once from its original position (`item.index` is the element's own index), then
writes each position. Receivers that name stored cells (descriptor arrays,
selected rows, dense arrays above `FIXED_CELL_UNROLL_LIMIT`) lower to one
`IrStmt::FixedArrayOrder`; selected-row reductions and `inside` items over stored
cells use `IrFixedArrayCells`. Never expand those receivers per element.

Resizable-container methods the packed/real callback runtime cannot express
(`containers/inline_methods.rs`: non-packed items, real/string keys, `with`
expressions that read activation state or call subroutines, string-keyed
index results) lower to one loop in the calling frame. Bind the iterator by
pushing an `InlineIterator`; element, key and `index` hooks consult it before
any other path, and pop it even on failure. Evaluate the `with` expression
exactly once per element in index/key order, record positions or keys in
activation queues and finish with `Gather`, `UniquePositions` or
`SortByKeys`; reductions are an `IrExprKind::Sequence` with an
identity-seeded accumulator. Stored one-dimensional packed/real fixed arrays
copy once into a queue (`fixed_method_copy`) whose declared range maps
positions to indices.
