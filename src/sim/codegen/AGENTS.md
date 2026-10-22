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
- A built-in without a lowering stays on the VPI path only when an application
  could provide it (SV 36.3.2); names that cannot be (clock resolution) are in
  `clock_resolution_reference`, lowered as clocking events and `disable iff`
  conditions and rejected at the call anywhere else. A system function in statement position
  (void cast) is a function call, never a VPI task. Keep
  `tests/fixtures/sim/feature_completion/sim_099/builtin_inventory.tsv` in step.
- VPI call actuals go through `lower_vpi_argument`: a `string`-formal string
  literal passes its packed image (a `vpiConstant`), a scope operand of
  `$sdf_annotate`/`$list` (admitted as metadata by `semantic.rs`) passes its
  catalog object with a placeholder value, and string variables are located
  errors. Every `$` built-in lowered natively is listed for the startup
  override check (`populate_vpi_builtin_calls`, decision S99-D6).
- Preserve imported fill operations. Unbased fills expand in context-determined
  arithmetic, comparisons, conditionals, assignments and arguments; self-determined
  concat/replication positions stay one bit. Ordinary case uses common width/sign.
  Preserve X/Z for copies, display and case/literal equality; Z otherwise behaves
  as X under the operation's rules. Wide div/mod/pow must survive context widening.
- `==?`/`!=?` wildcards apply only to converted RHS X/Z. Known mismatch beats
  unknown bits; otherwise unmasked LHS X/Z yields X. Bit queries evaluate once:
  countones ignores X/Z, isunknown detects both; counts are signed 32-bit,
  predicates unsigned one-bit. Real operands remain invalid here.
- `randcase` (SV 18.16) lowers to a `DeclLocal` holding `IrSysFunc::Randcase`
  (weights lowered self-determined, in source order, their exact-sum width
  `widest + ceil(log2(items))` bounded by the packed-value limit with a
  source-addressed error) followed by an ordinary exact `Case` over the signed
  index (-1 selects nothing). Bodies take the common statement path, so
  suspension, `return`, `disable`, `break` and `continue` need no randcase
  code; sets `IrRandomUse::threads`. Return-flow analysis treats it as able to
  fall through.
- `randsequence` (SV 18.17, `statements/randsequence.rs`) expands small
  productions in place as structured statements. Recursive productions, those
  reaching them, and shared ones above `RANDSEQUENCE_INLINE_ELEMENT_LIMIT`
  elements (unless a `rand join` interleaves them) become states of one
  dispatch loop (`Forever` over a `Case` on a state local) with a return state
  per multi-site production. Recursive generations push their formals,
  implicit variables, return state and repeat counters onto activation queues
  (packed, real through its bits, and strings in a string queue) and check
  `RANDSEQUENCE_DEPTH_LIMIT`. Two or more rules select with
  `IrSysFunc::Randcase { strict: true }`; one rule evaluates no weight.
  `rand join` binds every item (each names a production of one rule without
  if/case/repeat items, S16-D10), then steps with `IrSysFunc::RandJoinPick`;
  a rule's weight code block is a step of its own. `break` exits through `CtrlScope::RandSequence`;
  `return` jumps to the current production's exit. Unsupported forms (non-input
  formals, recursive or overlapping joins, unsaveable recursive values) are
  located errors. Decisions S16-D1 to S16-D10.
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
initial and always procedures (SV 9.2.2.2.1). Before every other startup
process (after the sampled-clock tick processes) spawn the `always`
procedures whose statement is an event control (`always @(e) s`,
`always_ff`), then the Verilog-2001 declaration-assignment processes, so
those procedures observe the time-0 changes of continuous assignments, port
connections, Verilog initializers and `initial` procedures (decision T0-D2;
SystemVerilog static initializers are no change, T0-D1). Carry exact always-kind and typed writes through validation;
writer, timing and flip-flop violations reject independently of lint. Ordinary
`always` remains a procedural loop with cooperative, source-located back-edge
budgets, not a comb/run-once replacement.

Preserve program-instance origin and Reactive launch (initials, generate-scope
initials and continuous assignments of a program), rejecting prohibited program
members (also inside program generate scopes) before lowering; `$exit` admission
requires program-process context.
Preserve source-origin classification and declaration lifetime rather than
qualifier spelling. Defaults precede initialization and processes: ordinary nets
start Z, arrays/scalar variables receive their typed fills/initializers, and
resolved defaults/delayed driver X contributions retain their separate phases.
A time-zero process write must win over initialization. Static locals initialize
once in hidden model storage; automatic locals initialize per lexical entry,
including inherited lifetimes and loop/block scopes.
Subroutine calls always use the typed `IrFunc`, timed or disabling included;
there is no call-site expansion (removed by SIM-009-A03). A `ref` formal read by
an event control binds a whole-signal actual statically: each distinct actual
signal gets one cloned task (`task_specialization`, lowered after all call
sites); otherwise the typed body follows whatever its descriptor names
(`IrDependency::RefFormal`): a variable, fixed-array element, class property,
container element (its container's contents marker), member view or unpacked
aggregate (range-precise leaves). Selects, qualifiers, named-event lists, `@*`
and controls reading a subroutine container are process-evaluated. A task
waiting on a native record `ref` formal is a template reached only through
record specializations (`waits_on_record_ref`). String and handle `ref` formals
wait on their actual's marker (`IrDependency::NativeRefFormal`); each call
checks its actual has one (`native_ref_actual_observable`). Keep those cases in
that one predicate and record new ones there. A fork-shared string keeps a
marker only when `native_observed` finds a wait that can observe it; waits on
fork-shared strings and handles use `IrDependency::SharedNative`.
An event evaluator reads fork-visible automatics through its context slots:
`event_context_with_aliases` maps the declaring subroutine's local (which
array-element reads name) to the slot, and a joined branch's borrowed capture
stays borrowed even when a detached fork elsewhere shares the cell.
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
initialize on each entry. Class-handle, semaphore, mailbox and string
initializers (module, package, `$unit`, static subroutine and class-method
locals, static class properties) and static native-record locals are
`IrInitStep::Execute` statements in the same schedule, never run-once
processes or first-call code; a handle initializer carries its instance as
`random_scope` so created objects seed from that instance's initialization RNG
(SV 18.14.1). `initialization/effects.rs` rejects a static initializer whose
calls (or constructor) reach a fork, nonblocking assignment or nonblocking
trigger (SV 13.4.4); `collection/const_writes.rs` rejects modifying built-in
methods on `const` declarations (SV 6.20.6, `Db::is_const_variable`). A name
redeclared in a package or `$unit` scope (a frontend warning) rejects instead
of aliasing storage.

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

Sequential UDP instances lower to one `UdpStep` at the start of the
continuous gate body followed by the shared publication of `UdpState`; each
instance owns one `IrModel::udp_states` record, so instance-array elements
keep independent states. The definition's typed rows come from the Db
(`core::db::UdpTable`, located rows); a row the IR cannot represent fails
with its source location. The initial value is published at time zero
without the instance delay: lowering sets `delayed_driver_start` so the
delayed driver starts at the initial value instead of X (S2-D4), and the
first step runs at time zero with every prior input X (decision S3-D4).

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
replacement. Preserve full single/rise-fall/three-way delays. A one-bit target
follows the from/to table (a change to X takes the smallest delay); a wider
target takes one delay for its whole value (nonzero to zero falls, all-Z turns
off, else rises; V §6.1.3). A net delay schedules the whole resolved vector,
or each separately driven part, with the delay selected by the new least
significant bit (S2-D3); `net_delay_layout` derives the parts from every
driver site after initialization steps exist and marks driven delayed nets and
module-path destinations to start X (S2-D10, A7-D6); every other driven wire,
wand or wor group records the bit runs its sites write (`driven_runs`), which
start X until the first driver write (T0-D3); a constant driver's z bits are
left out (T0-D4). A net
declaration assignment's delay is that driver's delay, never a net delay (the
capture gives it only to the assignment). Selected delayed drivers and
net-array cells start X on their own bits (`initialize_delayed_lhs`). A delayed
concatenation or alias-split target is one vector driver: one inertial driver
on a hidden packed signal plus a zero-delay scatter process
(`delayed_shadow_driver`), never one delay per part; pattern members are
separate drivers. A SystemVerilog runtime-valued driver delay
(`driver_delay`, `IrInertialDelay::Runtime`) is evaluated by the driver when it
schedules; a runtime net delay registers one `SetNetDelay` updater process,
emitted before every Comb process and sensitive to its operands. Verilog-2001
rejects both (V §7.14.1). Every min:typ:max expression, folded
or lowered, selects through `min_typ_max_operand`.
Simple module paths lower after the net groups and their structural driver
sites exist (`collection/module_paths.rs`): one `IrModulePathSet` per
instance, terminals resolved to storage-bit ranges, delays folded through
`path_delay_ticks` (negative is zero) and expanded to twelve transitions,
one hidden `ModulePathStamp` SensLoop process on the sources. A destination
must be a whole net group driven only inside the instance subtree (Net-member
slots of collapsed inout groups are storage, not drivers); IEEE 1364-2001
also allows one driver site. Timing checks, pulse styles, `PATHPULSE$`,
state-dependent and edge-sensitive paths and variable destinations fail with
their location. Specify-block specparams are collected with the instance's
parameters. A design without specify blocks costs one emptiness test.
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
simulation and retain packed/open-array/ref guards. Exports (`IrFunc::dpi_export`)
keep the import formal/result rules, reject time-consuming tasks (ADV-018),
reserved or colliding C identifiers and duplicate (scope, C identifier) pairs;
`finish_dpi` sorts `IrModel::dpi_scopes` by name (the runtime binary-searches
them) and import call sites carry `dpi_site` for `svGetCallerInfo` (S41-D3).
Static exported subroutines name their persistent formal storage in
`IrDpiExport::storage`, which the export thunk reads and writes around the call.

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
class method call, not even the handle variable, nor for a direct class-handle
reference such as `h == null` (§9.2.2.2.1, AB-N2;
`comb_excludes_class_references`); `@*` adds the handle identifier `h` of `h.x`
(§9.4.2.2: every identifier in the statement). Neither adds class storage
(§9.2.2.2.1); chandle event expressions reject (§6.14). always_comb,
always_latch and `@*` follow a member read through a virtual-interface handle
(`with_interface_reads`, S99-D7): a packed member is an
`IrDependency::NativeAccess`, an element of an unpacked array member an
`IrDependency::InterfaceInstance` (one fixed-array marker per bound instance),
both resolved through the handle each time the sensitivity loop or `@*` wait
arms, beside the handle variable the read walk adds; a member the block also
writes through the same handle is left out. Reads through a handle formal of a
called function, a vif-array element or another handle the arming process
cannot evaluate, and `v.f()` calls under always_comb, are located errors.
An element of an unpacked array member through a virtual interface
(`objects/virtual_interfaces.rs`, `v.mem[i]`) has no member slot: reads are a
`Mux` chain and `=`/`<=` writes an `IrStmt::Case` over
`IrObjectQuery::VirtualInterfaceInstance`, one arm per bound instance's array,
like the SIM-034 drives; other uses are located rejections. Copy supported automatic evaluator captures and
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
bounds; invalid views fail before writes. `statements/memory_tasks.rs` lowers
`$sreadmemb/$sreadmemh (memory, start, finish, string {, string})` use the same
lowering with the arguments rearranged to the `$readmem*` layout
(`lower_string_memory_task`): the data arguments, joined by one newline each,
become the `text` string expression of the load (`IrStmt::Memory`/`IrMemoryAccess`;
adjacent literals merge, so literal-only data stays one `Literal` compiled into
the model, and other data is evaluated after the bounds when the task runs;
an integral argument is `FromPacked`, S26-D17), and the path is only the
diagnostic label `$sreadmemh at <scope>:<line>:<col>`. Start and
finish are required, so the load direction never depends on the edition.
`$readmem*`/`$writemem*`: memories with model array storage keep the
fixed-array call; a slice with a variable position becomes a `SliceBase`
selector and a queue range slice with a runtime or `$` bound (or reversed
constant bounds) a `SliceLeft`/`SliceRight` pair, resolved once when the task
runs (S27-D19/D20); `this.<property>` names the property as the bare name
does; dynamic arrays and queues (whole or a slice checked at run time; with
fixed-array elements also one element through a leading `Row` selector;
a multidimensional class property of packed words is one fixed-size container
of all its cells in row-major order, recorded in `fixed_view_dims`: memory
files view it like a fixed array, whole copies, equality, `%p` and nested
positional patterns use it whole, and `reject_multidimensional_view` rejects
its element, row and slice selects), integral-index associative arrays (a wildcard index
rejects with its location) and fixed class properties use
`IrMemoryStorage::Container`; every other variable memory (subroutine and
block locals, `ref` formals, structure members) is a packed image
(`IrMemoryStorage::Image`) whose root is lowered once: as an lvalue for a load
(`IrMemoryImage::Target`) and as a value for a dump. The emitter resolves a
load's root once and loads a whole variable or local, or a constant member of
one, in place; any other root is copied and stored back with one blocking
assignment through the same resolved target (decision S27-D13). Binary rank-one reads advance numeric
addresses independently of declaration order. Packed `$fread` optional bounds
are evaluated/disposed but their values are ignored; memory bounds remain active.
File-input destinations (`expressions/external_input.rs`) evaluate selectors once
at call entry, except a `$fscanf`/`$sscanf` destination whose selectors read an
earlier destination of the call (`lhs_selectors_read_lvalue_writes`): it is
staged and selected by its store (S26-D1). String and real container elements
and container selects are staged through locals declared in an
`IrExprKind::Sequence`. The staged value is stored only when the call's result
exceeds that destination's ordinal.
Unpacked aggregates other than 1-D byte arrays reject before C generation.

## Constrained randomization

`randomization/` lowers `randomize()`, `std::randomize()` and the mode methods
(IEEE 1800-2009 clause 18; scope and decisions S29-D1..D10 and S30-D1..D11 in
[docs/lrm_decisions.md](../../../docs/lrm_decisions.md)). Each call site and
dynamic class case owns one static `IrRandProblem`; constraint expressions are
lowered by the ordinary expression path while `RandLowering` maps every random
variable to a `_llg_randvar_<k>` placeholder (`lower_expr`, `class_field_cell`,
receiver and `this` hooks), and `ProblemBuilder` turns the placeholder-bearing
IR into solver nodes. Subtrees without placeholders become state leaves read
once before the solve; logical operators keep their structure so guards
(18.5.12) can suppress an erroring leaf. Never let a constraint read random
storage directly: `check_direct_reads` is the backstop, and a constraint a
problem cannot express is a located error, never silently dropped.

`rand_mode`/`constraint_mode` state is hidden one-bit two-state storage per
random property and per root constraint block (overriding blocks share the
root's bit; static members use a static signal), invisible to `%p`. The
dynamic class is resolved through `DynamicCast` cases, most derived first; the
most-derived user `pre_randomize`/`post_randomize` (declared line != 0) runs
before modes and state are read and after the commit. A value-form randomize
is hoisted into `rand_prelude` only where it runs exactly once per statement
evaluation; short-circuit/conditional operands and loop conditions reject.
Commit writes only after a successful solve, so a failed call leaves every
variable unchanged (18.6.3).

Arrays, `foreach` and `rand` object members make a problem a template with
units (`templates.rs`, decisions S30-D5..D11): unit 0 is the randomized object
or `std::randomize` scope, each other unit a class an object reached through a
`rand` handle can have; variables, arrays and handle slots are problem-wide,
one per property declaration. `template_forms` runs before a constraint
expression is lowered and registers array selects, `size()`, reductions
(`item`, `item.index`), `inside` an array, members and null tests through
`rand` handles as `_llg_randvar_t<n>` placeholders that `random_placeholder`
returns; `check_template_uses` rejects a form lowered another way and a random
array read directly. The generated code walks the instances the runtime
registers (`pre_randomize`, variables, blocks, leaves, arrays, links), then
commits arrays and member objects and runs `post_randomize` in registration
order (S30-D7). Problems without these forms keep the flat SIM-029 shape.

## Functional coverage

`coverage.rs` lowers only covergroups that a `new` or an instance method call
names; a model without one gets no tables, processes or calls. Each lowered
group is a synthetic class (`class_nodes[group]`): field 0 is the runtime
record chandle, then the embedded owner, the `ref`-binding variant and one
field per input argument (`class_fields[formal]`). While a sample body or an
option lowers, `coverage_scope` makes argument reads go through the
covergroup object and `coverage_ref_actuals` maps each `ref` formal to the
variable bound by that variant; the owner is the method receiver, so owner
properties lower as ordinary class fields. Bins are resolved here (19.5.6 on
the operand of the frontend's conversion to the coverpoint type) into sorted
disjoint key segments; signed keys flip the sign bit. Transition bins become
compile-time automata in `coverage/transitions.rs`, whose `Tables::consume`
mirrors the runtime `llg_cov_consume`; it also sizes `[]` arrays, counts
coverable sequences and finds `detect_overlap` pairs within fixed budgets
(exceeding one is a located llg-limitation error). Cross select expressions
become DNF boxes over value-bin ranks (`cross_select`), at most
`MAX_SELECT_BOXES` per bin. Options are checked at their 19.7 levels
(`check_option`); type options must be constants. A clocking event on static
storage lowers to one sampler process spawned before user processes. An event
reading an argument or owner property becomes a waiter per instance
(`emit_covergroup_waiter`): a `CapturedBranch` helper of the sample method
whose only capture (slot 0) is the covergroup object, lowered with that object
as `coverage_scope` and the owner field as class receiver, and started by
`IrCoverageStmt::Watch` in `new`; `ref`-variant events dispatch on the bind
field. `type_option.strobe` turns the sample into a Postponed request.
Block events are injected into the named subroutine body in
`collection/function_bodies.rs` (a `return` there becomes a store and a jump
to the label before the end samples, `EmitCtx::coverage_end_exit`) and into
named blocks in `statements/dispatch.rs` (the event names the block's label
scope, resolved by `named_block_statement`); `check_coverage_block_events`
rejects any target that never reached either.
Diagnostics must stay located (`coverage_error`), and every coverpoint read
must stay an IR operand so `opt/passes/storage.rs` sees it.

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
design tick. A `$unit` without `timeunit`/`timeprecision` has no recorded scale
(a `timescale directive does not set it, SV 3.14.2.3): it uses unit 1ns and the
finest declared precision capped at 1ns, or 1ps when nothing is declared
(decision F2-D2). Frontend owns inheritance.

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
block, named fork (its target block), subroutine or generate block; in class
methods the class below the scope from `Db::class_declaring_scope`, S99-D10; an
`extern` body is named once, not also by its prototype, CD-D2), and non-`%p`
conversions of unpacked values
or handles reject. `%p` (`objects/patterns.rs`) lowers to `IrStringExpr::Pattern`
over interned `IrModel::pattern_types`/`pattern_classes`; the runtime walker in
`rt/scheduler/patterns.c` only reads storage, bounds object nesting with
`LLG_PATTERN_MAX_DEPTH`, marks cycles and reclaimed objects instead of following
them, and caps text at `LLG_PATTERN_OUTPUT_LIMIT`. Monitors/strobes run after settled Active/Inactive/NBA work; the latest `$monitor`
(plus any number of `$fmonitor` lists), registration/enabling forces a report,
time-only arguments do not retrigger. A deferred report in a static subroutine may
name model storage (static formals and locals included); activation-bound storage
(`activation_bound_ref`) and class methods reject. A monitor whose arguments read
handle-selected storage or `%p` a class object sets `IrStmt::MonitorSet::poll`:
the runtime then re-evaluates it at every settled slot (`LLG_MONITOR_READ_POLL`).
Deferred callbacks need owned environments. Waveform controls preserve source/depth
identity against the fixed catalog; a `$dumpvars` argument naming a variable must
match waveform storage after lowering (S47-D9), and waveform names drop the loop
generate construct level (`tb.g[0]`, S47-D10); omit waveform/libfst when unused, reject
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
match-item effects. Preserve Slang actual/default expansion. Clock flow is
structural (`assertions/clocks.rs`, SV 16.14.3), except that a `|->`/`|=>`
consequent takes the clock its antecedent ends on unless that clock is inside
parentheses (`Db::is_parenthesized_assertion`) or a sequence instance
(A13-D1); an assertion registers on its semantic leading clock (16.17.1). A clock on one
direct packed-signal edge stays an edge clock; every other clocking event
(event lists, `edge`, value changes, bit selects, clocking-block events, `iff`
gates off the leading position, `$global_clock`) is a planned pulse clock: a
hidden one-bit signal pulsed at most once per time step by a synthetic
process (`IrStmt::AssertionClockPulse`, `llg_assertion_clock_pulse`), shared
by equal event lists. `$inferred_clock`/`$inferred_disable` defaults resolve
at the instance (16.15.7). Locals (`assertions/locals.rs`)
are initialized at attempt entry when their instance starts the attempt (formals
first, then Db `assertion_local_initializer`s in declaration order); any other
instance with locals, and every output/inout formal, lowers as the SV 16.8.2
inlined form `(1, f = actual) ##0 body` plus a copy-out item on a transition of
its own. Property-level locals are forced into the antecedent graph and
inherited by the consequent (`PropertyLocalInit`); property-engine operands
reject them. Subroutine match items (`assertions/match_calls.rs`) lower like
deferred assertion actions: `IrExprKind::MatchCall` fills an owned frame in the
match callback and the callback is a `DeferredAssertion` pre-fn on a synthetic,
never-spawned process. Sequence end points (`assertions/endpoints.rs`) are
hidden `cover sequence` assertions with an `IrAssertionEndpoint`; `.triggered`
and `@(seq)` read its event, `.matched` its per-destination-clock slot.
Sequence composition chooses its mechanism per site: one-cycle operands of
`and`/`intersect`/`throughout`/`within` stay one sampled `&&` atom; multi-cycle
operands become an `IrSequenceJoin`. Repeated non-Boolean sequences re-lower
their body per literal copy (an unbounded bound loops the last copy), within
`SEQUENCE_STATE_BUDGET`; never introduce a fixed unroll depth for `$`.
Assignments to locals inside join operands and differently clocked join
operands stay explicit rejections (16.14.1 makes cross-clock ones illegal). A
property that the predicate/sequence forms cannot represent
(`property_requires_engine`) lowers whole to an `IrProperty` node table
(`assertions/property.rs`): every node and sequence carries its own clock from
the program's clock list (leading clock first), single-tick operands are
sampled atoms on their clock, and the `strong`/`weak` default comes from the
assertion kind. A recursive re-entry (16.13.17), which Slang leaves
unexpanded, becomes `IrPropertyNode::Call` of the nearest enclosing expansion
of the same property whose actuals have equal structural keys and whose
flowing clock is the same (Annex F.7); the call site's actuals are read from
any expansion of the same site. Recursion through local variable formals is a
located limit. A non-signal
`disable iff` condition, and an inherited `default disable iff` (Db
`default_disable`, not for `expect`), become a hidden net driven by a
synthetic process and shared per instance and condition node. `expect` action
blocks lower inline in the calling process after `IrStmt::Expect`, branching on
`IrRuntimeQuery::ExpectOutcome`; other action blocks are spawned processes
that may contain timing controls. A concurrent assertion reached in procedural
code (`assertions/procedural.rs`, SV 16.15.6) registers as an ordinary
assertion with a dense `IrAssertion::procedural` index and lowers to
`IrStmt::QueueProceduralAssertion`; without a clock of its own it takes
`infer_procedure_clock` of `Codegen::procedure_body` before default clocking.
`const'` casts, procedure automatics and an inferred `edge` clock are located
rejections (no queued values are kept), and its operands are not implicit
sensitivities (S99-D2). Assertion-only `initial` and timing-free `always`
procedures keep the static path. Deferred
assertions retain issue-time values (packed, real, string, chandle/class handle
captures; a method receiver naming static storage is read at maturity with an
issue-time index, others are captured, S36-D3) and Reactive actions under the single-call
contract in process, subroutine, class-method and final contexts, rejecting
automatic/dynamic ref actuals, timing actions and container/event/virtual-
interface action arguments. `disable` of a labeled deferred assertion or of a
procedure's outermost block also emits `DeferredAssertionDisable` (SV 16.4.4).
A module-item deferred assertion uses always_comb sensitivity (SV 16.4.3).
Mailboxes preserve typed copy/identity semantics and delegate waits/cancellation to
runtime queues. Aggregate messages (records, unpacked arrays, queues, dynamic
arrays) go through a lexical native value of the message type
(`collection/native_values/messages.rs`) and the SIM-003/SIM-006 transfers; their
type identity is the static equivalence key `mailbox_message_key` (SV 6.22.2), and
the runtime only compares it. Destinations that are not whole variables receive into
a local and copy out after the retrieval with selectors frozen at the call (a `ref`
argument, Annex G.4); expression-form `try_*` calls that need this are hoisted only
before assignments, system-task calls and `if` conditions. Whole packed, real or
string storage (a variable, an unpacked-record member held as its own leaf, a
class property of the object captured at the call, a formal or `ref` formal)
and a user class handle are `IrMailboxTarget::AtDelivery` destinations that a
blocked get writes at the put (`mailbox_target_at_delivery`, decision AB-O3). Container kinds keep
distinct storage, key conversion, notification and lifetime rules; another container's admitted operation is not a fallback.
The frontend flattens `c[i][b]` into one select: split indices at the container
depth. A packed select of an element is one read/modify/write with indices and
RHS captured once, written back through the whole-element store. Mailbox element
types come from the DB's captured `T`, never the rendered parameter spelling.
