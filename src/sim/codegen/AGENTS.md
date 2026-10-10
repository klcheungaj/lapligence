# Simulator lowering

Applies to `codegen.rs` and children. Consuming generation releases the DB after
typed lowering; borrowing entry points support reuse. Release collection state
before execution lowering and rendering. Read [pipeline](../AGENTS.md),
[emitter](../emit_c/AGENTS.md) and [runtime](../rt/AGENTS.md) contracts.
`lower_expr`/`lower_stmt`/`lower_lhs` produce typed IR only. Keep shared state in
`lowering.rs`, responsibility-named children and the smallest existing visibility.
[Feature status](../../../docs/sim_features.md) owns supported type/context
inventories; do not broaden a guard because another context accepts the syntax.

Keep raw source hierarchy components in `Codegen::c_paths`, independently of
scope lookup keys and historical display paths. Compose C names through the
helpers in `collection/names.rs`; encoded C fragments are never source path
components. Storage and runtime diagnostic labels use HDL metadata or owned source
components, including aggregate member paths and declared array bounds; never
decode C symbols.
Keep internal dependency sort keys separate from these display labels. Function
diagnostic labels remain separate from C symbols. The
[emitter guide](../emit_c/AGENTS.md) owns escaping, collision and length contracts.

## Values and selections

- Allocate exact-width values; `LLG_MAX_WIDTH` is the exclusive backend limit
  minus one, not model allocation capacity. Check constants, parameters, signals,
  concatenations, replications and intermediates; never truncate silently.
  Do not add detached C/`Verbatim` workarounds: the owned emitter rejects legacy
  fragments whose setup/cleanup cannot be represented.
- PLA system-task names have no selected legacy target. Diagnose them before
  the generic VPI task path; admitting their Verilog-2001 memory personality
  operand in the frontend does not select PLA execution.
- Preserve imported fill operations. Unbased fills expand in context-determined
  arithmetic, comparisons, conditionals, assignments and arguments; self-determined
  concat/replication positions stay one bit. Ordinary case uses common width/sign.
  Preserve X/Z for copies, display and case/literal equality; Z otherwise behaves
  as X under the operation's rules. Wide div/mod/pow must survive context widening.
- `==?`/`!=?` wildcards apply only to converted RHS X/Z. Known mismatch beats
  unknown bits; otherwise unmasked LHS X/Z yields X. Bit queries evaluate once:
  countones ignores X/Z, isunknown detects both; counts are signed 32-bit,
  predicates unsigned one-bit. Real operands remain invalid here.
- Get selectors, for-loop relationships, extents and delays from owned typed
  fields, not source names or frontend numeric codes. Widen signed/X/Z-aware
  coordinate arithmetic before subtraction/multiplication; unsigned high bits
  must not wrap into valid indices. Indexed widths/part bounds are static;
  bit/indexed bases may be runtime integral values. Missing metadata rejects.
- A range, indexed or element select addresses whole elements of the outermost
  packed dimension visible at its base; element width is the remaining
  dimensions times the element type (so records count whole). Map labels to bits
  only through `lowering/packed_geometry.rs`, never with per-site stride math.
  A member path whose first reference is an element select (`ps[i].hi`) names
  that select's root only as a label: resolve it through the select chain,
  never by the root's name, which would drop the select. Its selectors are
  reads of a writer (sensitivity and continuous re-evaluation).
- `PackedChain` preserves each selection relative to its preceding result,
  including remaining packed-element stride, bounds, direction and right bound.
  Partial reads keep valid bits and fill missing bits with X; invalid writes do
  nothing. Never flatten a chain to one unchecked root offset. Primitive-terminal
  projections recurse through the same typed steps, preserving per-step flags.
- Fixed layouts retain recursive shape, member identity, state domains, defaults
  and declaration order. Left dimensions vary slowest. Unpacked unions use their
  maximum storage width separately from streamed leaf order; packed members are
  one vector owner. Do not infer nominal identity from spelling or flattened size.
- Replicated patterns expand every dimension/position, even shared expression IDs.
  Check positive count and extent before allocation; do not expand twice.
  Resolve keys as expressions and preserve type/default precedence without
  assuming side-effect order. Type/default/replication evaluation multiplicity is
  undefined by SV 10.9.1; a single captured default before fan-out is one allowed
  choice. Descriptor patterns use lexical array snapshots rather than packed
  concatenations, retain uniform sparse defaults and loop over nonuniform repeats.
  Match packed keys against the owned immediate element type, preserving nominal
  enum identity and non-nominal integral width/range/sign/state equivalence.
- Lower array and direct unpacked-structure conditionals before flattening loses
  immediate boundaries. Use default-uninitialized values, not declaration member
  initializers. Capture selector once; known truth evaluates one arm, ambiguity
  evaluates both with type-specific merging. Array assignment snapshots all RHS
  cells before writes and retains explicit conversions/casts on fallback paths.
- Positional-pattern assignment snapshots the RHS, freezes all destination
  selectors before any write, then scatters. Fixed bit-stream casts convert the
  complete RHS, including nested state-domain conversion, before scattering;
  reference/inout identity is a separate contract. Continuous pattern leaves
  keep constant selects (runtime leaf selects reject) and each leaf is its own
  contribution: alias views, net-array cells and their selected bits publish
  through canonical alias bindings with a per-leaf terminal. Oversized sources,
  including typed patterns, scatter descriptor rows for every assignment kind.

## Processes, dependencies and initialization

Continuous assignments and comb/latch/implicit-sensitivity processes evaluate at
zero, then wait on RHS/body reads, not their LHS base. A zero-delay continuous
driver whose target storage, or another view of the same alias network, is
also an operand snapshots those operands and repeats in place while one
changed (`collection/continuous_feedback.rs`); a nonconvergent loop stops at
the process step limit. Combinational sensitivity
includes called-function reads and excludes written storage (a whole-array
read keeps its one contents marker above `PRECISE_EXCLUSION_CELLS` cells); plain
`@*` retains call-site behavior. String variables and string record members
contribute their change marker. always_comb/always_latch spawn after every
other startup process so their time-zero evaluation follows the start of all
initial and always procedures (SV 9.2.2.2.1). Carry exact always-kind and typed writes through validation;
writer, timing and flip-flop violations reject independently of lint. Ordinary
`always` remains a procedural loop with cooperative, source-located back-edge
budgets, not a comb/run-once replacement.

Preserve program-instance origin and Reactive launch, rejecting prohibited program
members before lowering; `$exit` admission requires program-process context.
Preserve source-origin classification and declaration lifetime rather than
qualifier spelling. Defaults precede initialization and processes: ordinary nets
start Z, arrays/scalar variables receive their typed fills/initializers, and
resolved defaults/delayed driver X contributions retain their separate phases.
A time-zero process write must win over initialization. Static locals initialize
once in hidden model storage; automatic locals initialize per lexical entry,
including inherited lifetimes and loop/block scopes.
Subroutine calls use the typed `IrFunc`, timed or disabling included; a call is
expanded (`lower_task_inline`) only where `CallShape` says the caller's environment
is needed: output/inout/ref event formals and event controls reading by-value
formals or locals; class-method event formals are typed. A `ref` formal read by an event
control binds a whole-signal actual statically: each distinct actual signal gets one
cloned task (`task_specialization`, lowered after all call sites); a level wait or
an explicit control without named events follows a whole-variable actual through
its descriptor (selects/qualifiers are process-evaluated), otherwise the call
expands. Keep those cases in that one predicate and record new ones there.
Fixed aggregate projections of static function locals use that persistent signal
as their read/write owner, including tagged-member guards; activation locals do
not name the persistent cell.

Collect parameters and concrete genvars with each generated path.
Subroutine names, prototypes and bodies traverse both generate arrays and
generate scopes recursively, including nested conditional/loop scopes and their
module instances. Fork references to input event formals resolve through typed
event captures before the enclosing task's formal bindings.
Replay deferred initializer calls/child references only after storage/prototypes exist, with
recursion depth reset; fail the whole replay on error. Declaration-call and member
default initialization finish before SystemVerilog processes start, never lazily
on first call. Reject unavailable lifetime/provenance or unrepresented native/
resizable initialization rather than fabricating storage.
Static initializers form one schedule (`lowering/initialization/order.rs`): an
initializer runs after every static declaration its value reads, directly or
through called functions (including their static locals), and otherwise keeps
the declaration order reserved at collection, so deferred and late-lowered
initializers keep their declaration slot. Package and `$unit` parameters are
collected before any module initializer folds them. Descriptor-backed fixed
arrays (module, package, function-static and block-static) initialize through
the typed pattern transport as `IrInitStep::Execute` in SystemVerilog and as an
active-region process in Verilog-2001; automatic descriptor locals declare and
initialize on each entry. A name redeclared in a package or `$unit` scope (a
frontend warning) rejects instead of aliasing storage.

Input links evaluate in the parent scope; selected outputs preserve untouched
bits. Constants/omitted defaults run once; explicit opens ignore defaults. Whole
packed/fixed aggregate refs retain canonical storage and member views without
copy links. Scalar real links use doubles and notify changed dependencies.
Interface/modport bodies use the actual Slang-bound instance; a simple modport
port resolves to its same-named interface member. Connection indices must be
elaborated constants (including genvars), except a variable output target: it is
an implied continuous assignment (SV 23.3.3.2) whose link re-evaluates its
selectors. The local Slang patch admits runtime selects only there and on
continuous assignments to variables (SV 10.3, A.8.5), whose process also waits
on its target selectors (`continuous_assignment_reads`) and whose writer set is
the longest static prefix (`add_continuous_lhs_write`). Ref, net, inout and gate
terminal connections keep constant selects. A ref actual that selects one leaf of a
record lowered as a concatenation binds to that leaf
(`collapse_concat_reference`), never to an unnamed projection. Descriptor-backed fixed-array ports and
nested member-array outputs reuse the procedural fixed-array assignment owner
(`FixedArrayCopy`/`FixedValueAssign`); never expand a descriptor port per cell.
Precollect hierarchical actual dependencies per instance.

Variable-continuous conflict analysis follows canonical intervals and counts
ordinary assignments/declaration initialization, not force/release/deassign.
Record and hierarchical member selects keep their declaration's storage class.
Constant rows of at most `PRECISE_ROW_WRITE_CELLS` cells write only their
cells for every writer; a wider row is one flattened cell interval (a
writer-analysis `PackedRange` over `ArrayContents` that `ir_process_writes`
widens before the IR), so the pairwise check costs one record per row.
Output ports connected to variables are continuous drivers: constant rows of
dense arrays drive their cells, runtime selects drive the longest static prefix,
constant rows and contiguous constant slices of descriptor storage drive one
cell interval (a slice whose cells are not one interval stays unregistered
rather than a false whole-array conflict); keep disjoint
writers legal and preserve original read sensitivities separately. Zero-delay
continuous drivers of whole descriptor arrays or constant descriptor rows use
the fixed-array assignment owner, never a flattened packed driver.
The always-family single-writer rule counts ordinary assignments only;
force/release and procedural `assign`/`deassign` are overrides another
process may apply. always_ff admits event triggers and these overrides; only
event-control count, blocking timing and forks are restricted.
Hierarchical structural driver identity includes owner, source and group.

## Nets and procedural drivers

Collapse inout nets to canonical electrical storage; emit no value-copy link.
Input/output links feeding a group retain their own structural driver sites.
Resolve net-type/delay selection through `net_collapse`/`port_net_types` before
storage union; same-net-type alias checks precede more permissive port rules.
Parent connections precede descendants, and each hierarchy depth collapses as
one `port_batch` against pre-batch types, so instance/port order never selects
a type: winners reduce to the non-dominated set, a warning-only tie takes the
first Table 23-1 column with a located warning, and every winning declaration
of that type owns the delay. A uwire in a collapsed network keeps one
structural driver per bit network (`note_uwire_drivers`, conservative for
unmapped targets); the frontend patch stops counting the inout connection
itself as a uwire driver, for each uwire operand of a concatenated actual and
for the internal net of an `inout uwire` formal.
Whole connections keep declaration-level metadata; selected connections map
physical electrical bits. Reject incompatible/non-net members and unsupported
resolution/sensitivity, never recover an unresolved driver from source text.

Give continuous/declaration/gate sites deterministic independent contribution slots,
sized at elaboration without a fixed driver ceiling. Selected contributions restart
from Z each evaluation. Synthetic slots have no waveform names; reads/dependencies
observe resolved cells only. Wire/wired/pull/supply resolution retains both
strength endpoints of X; a known drive wins only when stronger than every opposite
possibility. High-Z adds no drive. Wired-AND 0 and wired-OR 1 dominate X; all-Z
sources resolve Z unless a default source applies. Pull/supply defaults are
strength-bearing, not unconditional values. Preserve scalar strengths; explicit
vector (including selected-bit) and supply-net continuous strengths reject; gate
instance-array strengths stay legal. Undelayed enable gates split into a
strength0-only and a strength1-only slot so an unknown enable drives L/H; delayed
ones keep one slot (X) so each transition keeps a single delay. An omitted input
under `unconnected_drive` is a pull-strength slot of its port on every net or
net-array formal (variables receive the value only); input net-array links and
gate outputs on net-array cells publish through their port/gate cell slots.
`%v` on a net, its constant projections, aliases or net-array cells reads a lazily
created strength-view signal (`collection/strength_views.rs`); monitors depend on
that view, so strength-only changes reach them without waking value readers. Reject trireg before storage collection,
including undriven arrays, and dynamic net selectors. Keep disjoint constant
uwire selections legal; overlapping drivers reject.

Combinational UDP instances lower scalar input expressions directly into
`UdpEval` (no per-evaluation input locals) and intern each definition once per
model through a hash of its typed rows. Terminals select from larger objects
through the ordinary expression/lvalue paths; a bit select of a computed value
(operator, conditional, cast or slice) is numbered by that value's own packed
type, as Slang's instance-array slicing requires. Vector or aggregate terminals
of a single instance reject.

`collection/net_partition.rs` forms maximal affine runs of canonical electrical
roots for fixed arrays and selected/aliased scalar or vector nets. Adjacent
positions must have the same declared members, structural source/terminal sites,
force/release membership and resolved net type/delay owners; each member and
source offset advances by one. Reversed/permuted or opaque projections retain
bit groups. Include driver ranges before partitioning, and extend that source
inventory for new topology forms. Ordinary whole vector groups already resolve
word-parallel and retain their existing path. Keep per-bit declared-view bindings
with a group-bit offset (the emitter renders consecutive bits as one run-length
alias part); electrical width must never change an HDL storage shape.
Whole net-array inout ports, rows and slices union each formal cell with its
left-to-left actual cell (`net_array_inout_pairs`) in both storage and type plans.
`collection/net_cells.rs` classifies cells before that: a cell joins the
bit-level path only when a structural source, true alias, selected inout
connection or force/release target reaches it or a whole-array inout peer of
it; a new topology form must be added to its inventory too, and publication
rejects a driver reaching an unclassified cell.
Contribution projection gathers contiguous RHS runs with part selects and Z runs,
then concatenates only disconnected runs. Conversion precedes projection. Array
publication binds every bit to this partition and rejects a missing root.
Record net-array member selections map declared cell coordinates and recursive
member offsets through `array_net_selection` before this same partitioning;
member boundaries split runs when their structural driver sites differ.

Fixed net arrays, selected ports and interfaces share canonical bits; array
publication notifies array dependencies. Keep the signal reverse index aligned
with every insertion and rebuild it after port canonicalization. Alias whole-net
views resolve selected positions lazily; retain eager first-error behavior when
endpoint validation cannot prove the complete affine range valid. True aliases
flatten MSB-to-LSB, with
ascending indexed-minus starts checked as base-width+1 before materialization.
Retain self/duplicate/net-kind/cross-scope restrictions. Hierarchical/concatenated
continuous targets use resolved sites, not function/task-output or procedural-net
write paths.

Delayed contributions start X; genuinely driverless nets start Z. Inertial updates
own one Active event per site/array element without suspending their evaluator.
Compare pending contribution values, not resolved nets; unchanged values keep
original deadlines, changed values cancel, return to current value cancels without
replacement. Preserve full single/rise-fall/three-way delays and choose each
changed bit's delay, taking minimum applicable endpoints for ambiguous transitions.
Force overlays visible storage while ordinary writes/driver slots remain live;
release retains variable value or recomputes current net resolution. Releasing a
variable toggles its hidden release marker; `attach_release_reevaluation` makes
the variable's synthetic continuous drivers (assignments, port links) sensitive
to it after all lowering, so they rerun (SV 10.6.2). Re-force replaces a matching
entry per bit. Keep typed constant-selected/concat targets; constant indexed,
member and element selects of nets become fixed parts (`normalize_force_lhs`).
Reject unsupported variable selects, automatic/local captures, arrays, and
string/handle/event or whole net-array targets with located errors.
Procedural `assign`/`deassign` is unsupported by design: `legacy_unsupported::scan`
rejects every reachable statement before lowering and dispatch rejects the rest.
No subroutine/captured RHS admission without an owned activation environment.

## Calls, loops and callbacks

Capture each input once before dependent defaults. Preserve exact fixed activation
payloads; automatic locals/returns are independent, static formals/locals/returns
persist definition-wide. Input/inout copy in per call, output/inout copy out at
return; freeze composite destinations before invocation. Explicit local lifetimes
apply individually (static-in-automatic and automatic-in-static). Static output
formals are not implicitly reset/copied in. Static return storage is ordinary
hidden model storage, not an opportunistic expression substitute.

Ref/const-ref forward original leaves through selected views/nested calls; never
invent globals for automatic formals. Inline delay/wait-bearing tasks; reject
recursive timed tasks and task calls from functions. NBA targets must outlive
publication: fixed static subroutine arrays use persistent per-instance signal
storage and typed selections; automatic/loop-local and unsupported native
subprogram storage reject.
String/chandle/native paths need typed owners/captures; chandles stay `void *`,
never integers. Preserve explicit C names and pure/context qualifiers in canonical
`svdpi.h` scalar thunks; reject missing/conflicting libraries/signatures before
simulation and retain packed/open-array/ref/export/context-callback guards.

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

## Events, predicates and streams

Register event or-lists atomically with `llg_wait_any_events`, never sequential
waits. True waits execute once immediately; false/X/Z constant waits suspend on
empty dependencies without polling or blocking time advancement. Evaluated events
compare expression values, not every operand change; qualifiers run at trigger.
Packed edges use LSB, real any-change uses IEEE bits (signed-zero changes wake;
identical NaN payloads do not). Storage reached through a handle (class
properties of every direct kind including strings and handles, handle-property
chains, virtual-interface members) is an `IrDependency::NativeAccess` prepared
per wait root by `with_dynamic_reads` (`collection/dynamic_reads.rs`). Called
function and method bodies are walked with class-handle formals bound to the
lowered actuals and `this` to the receiver (`CalleeReceivers`); a method body's
module storage reads are added there, since the ordinary read walk does not
enter methods. A read no wait can observe rejects with its location, never
compiles to a wait that cannot wake (a missed dependency is a hang). Only waits that
re-arm after every wake (`wait`, process-evaluated event plans) may use it, so
such event controls, foreign functions and `ref`-formal selects take the
process plan. always_comb/always_latch add nothing for a class property path or
class method call, not even the handle variable (§9.2.2.2.1;
`comb_excludes_class_references`); `@*` adds the handle identifier `h` of `h.x`
(§9.4.2.2: every identifier in the statement). Neither adds class (§9.2.2.2.1) or
virtual-interface (§25.9) storage; chandle event expressions reject (§6.14). Copy supported automatic evaluator captures and
transitively reject disallowed callback effects/captures. Named/mixed events retain
identity and atomic registration. `->>` captures delay at issue and queues NBA;
event/repeat timing uses an independent detached waiter, whose final trigger is
still NBA. Every repeat count (loop, intra-assignment, `->>`) goes through
`lower_repeat_count`: real counts convert by rounding (SV 6.12.2) and the
runtime's `sv4_repeat_count` maps X/Z and nonpositive signed counts to zero.
Blocking delay and event intra-assignment timing share
`lower_blocking_timed_assignment`: the RHS (or each record leaf, or a container
element's value) is captured before the wait and the destination store runs
after it. Event-controlled string/handle NBAs copy the value at issue into a
detached process frame (`lower_native_event_nba`) that waits through
`ProcessEventPlan` and then queues the native NBA.

Sequential predicates evaluate clauses once, left-to-right, continuing only on
definite true. Preserve explicit branches/pattern bindings and short-circuit
unreachable constants; never replace with `&&`. Matching sources/selectors are
captured once; filters use per-item bindings and first-match/default order.
Tagged pattern checks use the enclosing case mode; ordinary member guards are exact.
Packed and finite unpacked tagged unions are one packed owner (tag in the MSBs,
members right-justified); records, unions and arrays nested in a member use the
flattened fixed-value order. Member paths and their selects go through fixed
projections so each step keeps its tag guard; never select payload bits unguarded.
Recursive constant checks read each structure member in its own
state domain, including two-state conversion inside a four-state packed record.
Constant-pattern operands admit fixed integral atoms, packed structures and
untagged packed unions; reject unpacked, real and native shapes before scalar
comparison without changing their owned descriptors.
`inside` follows expression result types before storage roots, keeps
casts, descends arrays to singulars (not struct members), evaluates selector and
scalar/range endpoints once, and uses wildcard equality/inclusive comparisons.

Integral fixed arrays beyond packed capacity lower to `IrFixedValue` operands
(views, calls, conditionals, streams, converting casts) and `FixedValueAssign`;
never expand them per cell or flatten them into one packed value. Descriptor
formals carry their storage in `IrFormal::fixed_array`; returns use a trailing
output formal. Pattern items and scatter sources are captured once into lexical
snapshots before writes. Never look through a cast that reshapes cells or enters
a two-state element domain: it is an `IrFixedValue::Convert` into a lexical shape.
Equality whose operands are all descriptor values also uses descriptor compare
below packed capacity rather than flattening each cell.
The fixed-value/view/projection IR is frozen at the RTL-099 gate
([contract](../ir/readme.md#frozen-fixed-value-view-and-projection-contract-rtl-099)).
SIM-wave migration notes: native/resizable aggregates get their own typed
descriptors next to it rather than new `IrFixedValue` element domains; a
consumer that newly accepts a fixed aggregate must take the descriptor path
for `sparse()` storage and oversized values, never a per-cell expansion; new
writers of fixed storage must register cell intervals for the single-writer
rule; and new net-array driver forms must be added to the `net_cells.rs`
classification inventory.
Column-layout records and tagged unions (RTL-101; wider than the packed limit
or with a member array above the dense threshold) are classified in
`collection/record_columns.rs` and lowered in
`expressions/aggregates/columns.rs`: every integral leaf is an
`IrArray::descriptor` column (a scalar leaf is one cell, a tag is one more
cell), whole-value operations iterate the columns, subroutine activations keep
their columns in `activation_records` (extra columns follow the declared
formals, results are trailing outputs), and a consumer that reads a member
must resolve it through `record_column_array` rather than a packed projection.
Real/string/chandle leaves of an activation record are `RecordColumn::Native`
items of one native value, passed after the columns. Static initializers go to
`record_initializers` (static schedule); automatic block records are lexical
(`automatic_block_record`). An operand that needs setup inside an expression
(a record call result, a whole-value binding) is an `IrExprKind::Sequence`
whose statements never suspend; binding storage is declared before the
enclosing statement through `record_binding_declarations`, opened by
`lower_stmt`. Whole tagged-union member copies check the tag with
`record_guard_check`, the same runtime report as element accesses.
Native records (string/real/chandle leaves, no packed width) in subroutine
formals, results and locals use `collection/native_values.rs`: one
`IrNativeValue` root per storage, leaf accesses by constant item path, and
endpoint transfers that capture every source leaf before the first write.
A scalar member of a native-result call (`f(x).s`) or a nested record
pattern item from a call runs the call into a lexical temporary first
(`native_value_of`); in an expression that setup is an `IrExprKind::Sequence`
(a string member reads through a `Conditional` whose predicate is the
sequence).
Module-level native records keep per-member lowering: NBAs, continuous
assignments and conditional merges go leaf by leaf, capturing every source
leaf first (`objects/assignments.rs`, `statements/native_delays.rs`,
`native_values/conditionals.rs`). Records with native or container leaves
declared in process blocks get the same leaf storage, collected per
declaration and instance with the scope (`collection/block_records.rs`): static
ones initialize in the static schedule, automatic ones are reset and
initialized at their declaration statement, and an automatic one that a
`join_any`/`join_none` fork could keep live across re-entry rejects
(`concurrent_block_activation`: an enclosing detached fork that runs again,
or a block that runs again starting one that reads it). Block-declared
strings, chandles, class handles and virtual interfaces get one model object
per declaration and instance the same way (`collection/block_natives.rs`),
resolved only by declaration identity, so change markers, NBAs and calls
behave as for module variables. Native record leaves without a declaration
initializer take their type's member defaults (`collection/record_defaults.rs`;
the outermost default on a path wins); a module record initializer that is
not a leaf-wise pattern is one record assignment in the static schedule.
String object values (`IrObject::initial`) join that schedule at their
declaration's slot. A native record `ref` formal takes a whole subroutine
record by address; a module, static or block record actual, or a constant
member/index selection of one, selects a specialization of the callee
(`native_values/record_refs.rs`, keyed with `task_specialization`) whose body
resolves the formal to that record's leaves (an `unpacked_aggregates` alias
installed while the body lowers), so the formal behaves as the record itself;
forwarding a bound formal rebinds the same record. Element, run-time-indexed
and subroutine-member actuals and method formals reject. Reject
run-time native indices, NBAs to native roots (their
leaves move on replacement) and fork capture explicitly rather than
flattening or copying shallowly. Native NBAs only target persistent whole
variables or module and static block record members; string bytes, class
properties and automatic storage are illegal targets (SV 6.21).
Reals stay numeric (SIM-005): real `ref` formals bind the actual's cell
(`collection/arguments.rs`), real fixed-array formals/results/locals are real
`IrArray` storage (`collection/real_arrays.rs`), and non-storage real-array
values lower to per-element expressions (`containers/real_array_values.rs`,
bounded by `REAL_ARRAY_ELEMENTWISE_LIMIT`). Never encode a real as packed bits
except as the exact 64-bit image a sampled history or a clocking-input
expression image transports.
Clocking inputs (SIM-033, `clocking_context.rs`): a clockvar bound to a plain
reference samples that signal; any other expression (select, concatenation,
computed, real) gets a hidden image signal kept equal to it by a synthetic
process plus a declaration-phase initializer, so Preponed at time 0 already
sees it. Every clockvar has one static sample cell of its own type; reads of
`cb.x`, its members and selects resolve to that cell, writes (outputs/inouts)
to the source. Sample types are limited to packed, real and shortreal.
Resizable containers in subroutine, procedural-block and class storage
(SIM-006) use `collection/subroutine_containers.rs`: storage per (instance,
declaration), bound into `container_globals` for the body being lowered, with
activation containers declared at their declaration statement. A function's
result container is visible only inside its own body (`container_of`). Calls
copy container inputs in and outputs back (`IrCallArg::Container`); a pattern
actual in expression position becomes `ContainerValues`, other non-variable
actuals need a statement prelude. Record elements go through
`collection/native_values/elements.rs` (whole-element transfers via native
temporaries, member leaves via element accesses). An element keeps a queue
or dynamic-array member as a nested dynamic array inside its value, unlike a
module record (own container) or native value (companion container): element
temporaries get companions, filled after `GetValue` and copied back before
`SetValue` (`IrContainerStmt::{ValueItemToContainer, ContainerToValueItem}`);
an assignment, call or system-task statement naming an element's member
stages it in a lexical container (`stage_element_members`), copied out before
the statement and back after it when the statement can change it, so every
container operation applies unchanged. Associative members and other
contexts reject explicitly. A read-modify-write of one
element (`c[k] op= v`, `c[k]++`) needs an index free of side effects; an
implicit conversion around a container index is stripped so X/Z bits reach the
runtime's invalid-index check. Reject `ref` container formals (SIM-008), fork
capture of activation containers (SIM-010) explicitly. A class container
property selected through an explicit handle (`h.q`, `n.next.q`) gets a
receiver-qualified alias container before its statement lowers
(`qualify_class_containers`, keyed by the path expression; element selects
find it through their path child); only plain handle reads qualify, other
receivers reject. Packed and integral-record class properties are
fixed-value cells (`class_field_cell`) for member, bit and part projections.
Explicit class receivers are `IrChandleExpr::Required` with their source site.

An invalid index reads the element type's uninitialized default (SV 7.4.6), not
an all-X payload: mixed-state aggregate elements keep two-state leaves at zero
(`IrArray::element_uninitialized`, projection leaf-state reapplication). A failed
task-form `$cast` reports a run-time error; the function form only returns zero.

Streams retain typed direction, slice size, operand order, unsigned result and
component LHS widths. Snapshot RHS once. Fixed `with` operands are one-dimensional
only, not multidimensional arrays or decorated nested concats; traverse declared
storage order. Never drop a `with` range: model arrays use `FixedStream`/
`FixedSelector`, other fixed arrays (ref formals, locals, members, rows, call
results) use their whole image (`FixedImageStream`/`FixedImageSelector`), and
constant in-bounds target ranges unpack into element parts. A runtime or
out-of-bounds target range needs the checked `StreamAssign` (`stream_mixed_targets`):
a direct streaming assignment, or a statement call's output copy-out
(`lower_call_writeback`, selectors frozen at the call); expression calls and
intra-assignment delays reject. Image elements mixing two-state and four-state
members carry their two-state runs: constant parts become one lvalue per run,
runtime selections convert the selected segment. Each blocking `>>` selector may
observe earlier unpacked fields, unlike frozen positional targets; nonblocking,
`<<` and copy-out forms reject a selector that reads an earlier target of the
same unpack. Oversized descriptor stream operands are views (constant in-bounds
`with` ranges as sliced views), `Selected` runtime `with` selections of
descriptor arrays, `Dense` views of dense arrays, `Packed` values (records,
`with` selections of dense/image arrays) or nested streams (`>>` spliced);
dense rows also enter descriptor patterns as `Dense` items and receive scatter
rows by a copy loop (`dense_row_scatter`). Positional pattern lvalues place
each row target, at any nesting depth, at its declaration-order cell prefix:
an oversized or descriptor source, or a descriptor row target, scatters from
one snapshot (packed leaves and net rows take packed cell images); otherwise
small sources split one bit stream over packed leaves and joined row images
(IEEE 1800-2009 10.10). Wider fixed
destinations left-align and zero-fill; oversize fixed sources diagnose, while
longer unpack sources consume leftmost bits. Composite call/ref layouts use the
same recursive descriptors.
Memory views evaluate each dynamic selector once, retaining static strides and HDL
bounds; invalid views fail before writes. Binary rank-one reads advance numeric
addresses independently of declaration order. Packed `$fread` optional bounds
are evaluated/disposed but their values are ignored; memory bounds remain active.

## Time, output and native services

Use the owned units/precision of the nearest module instance, package, `$unit` or
class-declaring scope (`Db::declaration_time_scale`) and typed `IrDelay`; round complete real
delays once locally before checked scaling. Packed X/Z delay becomes zero, negative
packed values convert to unsigned 64-bit time, and finite negative real delays
convert after local rounding (constants take the runtime conversion); reject
nonfinite real delays and tick overflow. `timescale.rs` owns tick representation; keep the runtime timescale-
agnostic. `$time`/`$stime` round to calling units with halves upward; `$realtime`
keeps fractions. `%t` carries physical units through runtime `$timeformat`;
`$printtimescale` reports its instance/`$unit` operand, or the scope containing
the call, in the SV 20.4.1 format. Code-bearing packages and `$unit` join the
design tick. Frontend owns inheritance.

Real/shortreal use typed doubles and f32 rounding at shortreal writes. Real-to-packed
rounds nearest, ties away; packed-to-real zeroes X/Z positions at actual width.
`$rtoi` truncates to signed 32-bit (nonfinite X, finite overflow modulo 2^32);
`$itor` preserves packed width/sign and rounds real input through signed 32-bit.
IEEE bit-reinterpretation inverses require 64/32 bits and zero X/Z. Typed constants
share conversions. Validate all 21 Table 20-4 math functions, evaluate each argument
once and retain libm domain/nonfinite behavior. Do not infer real expressions from
source or bypass context-specific rejection guards.

Parse display formats during lowering (`objects/format_text.rs`, shared by every
display, file, string and severity task); `%d` retains signedness and `$write` omits
newline. Arguments stay typed: never re-parse rendered text. Literal string
arguments start format segments, `%m` comes from `format_scope` (nearest named
block, subroutine or generate block), and non-`%p` conversions of unpacked values
or handles reject. `%p` (`objects/patterns.rs`) lowers to `IrStringExpr::Pattern`
over interned `IrModel::pattern_types`/`pattern_classes`; the runtime walker in
`rt/scheduler/patterns.c` only reads storage, bounds object nesting with
`LLG_PATTERN_MAX_DEPTH`, marks cycles and reclaimed objects instead of following
them, and caps text at `LLG_PATTERN_OUTPUT_LIMIT`. Monitors/strobes run after settled Active/Inactive/NBA work; the latest `$monitor`
(plus any number of `$fmonitor` lists), registration/enabling forces a report,
time-only arguments do not retrigger. A deferred report in a static subroutine may
name model storage (static formals and locals included); activation-bound storage
(`activation_bound_ref`) and class methods reject.
Deferred callbacks need owned environments. Waveform controls preserve source/depth
identity against the fixed catalog; omit waveform/libfst when unused, reject
`$dumpports`, warn/skip `$displayon`/`$displayoff`. Packed string literals remain
unsigned MSB-first byte vectors (empty is one zero byte), with normal pad/truncate;
SystemVerilog strings remain separate native owners.

Sampled-value functions intern one `IrSampledClock` per lowered clocking event
and gate and one `IrSampledDomain` per (clock, sample expression), keeping the
deepest `$past` depth, so overlapping calls share history. A single direct
packed-signal edge is an `Edge` clock; any other legal event list is an `Event`
clock ticked by a synthetic `WaitEvents` + `SampledClockTick` process spawned
before other time-zero processes. `iff` and the `$past` gate read current
values. Classify argument types from the Db before lowering: §16.6.1-excluded
types are illegal, unpacked/queue values are explicit unsupported; never coerce
a non-integral value into packed bits (reals use their exact 64-bit image).

Concurrent assertions use dedicated instances, immutable sampled predicates,
explicit clocks/disable/control metadata, per-attempt/thread locals and ordered
match-item effects. Preserve Slang actual/default expansion; reject conflicting
clocks, unsupported temporal/cross-clock forms or unrepresented copy-out.
Sequence composition chooses its mechanism per site: one-cycle operands of
`and`/`intersect`/`throughout`/`within` stay one sampled `&&` atom; multi-cycle
operands become an `IrSequenceJoin`. Repeated non-Boolean sequences re-lower
their body per literal copy (an unbounded bound loops the last copy), within
`SEQUENCE_STATE_BUDGET`; never introduce a fixed unroll depth for `$`.
Assignments to locals inside join operands and differently clocked join
operands stay explicit rejections (SIM-039, ADV-013). Deferred
assertions retain issue-time values and Reactive actions under the single-call
contract, rejecting unowned automatic/dynamic refs or timing/control actions.
Mailboxes preserve typed copy/identity semantics and delegate waits/cancellation to
runtime queues. Aggregate messages (records, unpacked arrays, queues, dynamic
arrays) go through a lexical native value of the message type
(`collection/native_values/messages.rs`) and the SIM-003/SIM-006 transfers; their
type identity is the static equivalence key `mailbox_message_key` (SV 6.22.2), and
the runtime only compares it. Destinations that are not whole variables receive into
a local and copy out after the retrieval with selectors frozen at the call (a `ref`
argument, Annex G.4); expression-form `try_*` calls that need this are hoisted only
before assignments, system-task calls and `if` conditions. Container kinds keep
distinct storage, key conversion, notification and lifetime rules; another container's admitted operation is not a fallback.
The frontend flattens `c[i][b]` into one select: split indices at the container
depth. A packed select of an element is one read/modify/write with indices and
RHS captured once, written back through the whole-element store. Mailbox element
types come from the DB's captured `T`, never the rendered parameter spelling.
