# Simulator lowering

Applies to `codegen.rs` and children. Read [pipeline](../AGENTS.md),
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
  assuming side-effect order. Capture a default once before fan-out.
- Lower array and direct unpacked-structure conditionals before flattening loses
  immediate boundaries. Use default-uninitialized values, not declaration member
  initializers. Capture selector once; known truth evaluates one arm, ambiguity
  evaluates both with type-specific merging. Array assignment snapshots all RHS
  cells before writes and retains explicit conversions/casts on fallback paths.
- Positional-pattern assignment snapshots the RHS, freezes all destination
  selectors before any write, then scatters. Fixed bit-stream casts convert the
  complete RHS, including nested state-domain conversion, before scattering;
  reference/inout identity is a separate contract.

## Processes, dependencies and initialization

Continuous assignments and comb/latch/implicit-sensitivity processes evaluate at
zero, then wait on RHS/body reads, not their LHS base. Combinational sensitivity
includes called-function reads and excludes written storage; plain `@*` retains
call-site behavior. Carry exact always-kind and typed writes through validation;
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
is needed: output/inout/ref event formals, event controls reading by-value formals
or locals, and class/virtual-interface event calls. A `ref` formal read by an event
control binds a whole-signal actual statically: each distinct actual signal gets one
cloned task (`task_specialization`, lowered after all call sites), otherwise the
call expands. Keep those cases in that one predicate and record new ones there.
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

Input links evaluate in the parent scope; selected outputs preserve untouched
bits. Constants/omitted defaults run once; explicit opens ignore defaults. Whole
packed/fixed aggregate refs retain canonical storage and member views without
copy links. Scalar real links use doubles and notify changed dependencies.
Interface/modport bodies use the actual Slang-bound instance. Connection indices
must be elaborated constants (including genvars), not dynamic connections.
Precollect hierarchical actual dependencies per instance.

Variable-continuous conflict analysis follows canonical intervals and counts
ordinary assignments/declaration initialization, not force/release/deassign;
keep disjoint writers legal and preserve original read sensitivities separately.
Hierarchical structural driver identity includes owner, source and group.

## Nets and procedural drivers

Collapse inout nets to canonical electrical storage; emit no value-copy link.
Input/output links feeding a group retain their own structural driver sites.
Resolve net-type/delay selection through `net_collapse`/`port_net_types` before
storage union; same-net-type alias checks precede more permissive port rules.
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
vector continuous strengths reject. Reject trireg before storage collection,
including undriven arrays, and dynamic net selectors. Keep disjoint constant
uwire selections legal; overlapping drivers reject.

`collection/net_partition.rs` forms maximal affine runs of canonical electrical
roots for fixed arrays and selected/aliased scalar or vector nets. Adjacent
positions must have the same declared members, structural source/terminal sites,
force/release membership and resolved net type/delay owners; each member and
source offset advances by one. Reversed/permuted or opaque projections retain
bit groups. Include driver ranges before partitioning, and extend that source
inventory for new topology forms. Ordinary whole vector groups already resolve
word-parallel and retain their existing path. Keep per-bit declared-view bindings
with a group-bit offset; electrical width must never change an HDL storage shape.
Contribution projection gathers contiguous RHS runs with part selects and Z runs,
then concatenates only disconnected runs. Conversion precedes projection. Array
publication binds every bit to this partition and rejects a missing root.

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
release retains variable value, resumes PCA or recomputes current net resolution.
Re-force replaces a matching entry. Keep typed constant-selected/concat targets;
reject unsupported variable selects, automatic/local captures and arrays.
Procedural assign uses pre-scanned enable-guarded sites; deassign retains value.
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

Fixed reductions map immediate elements rather than recursively flattening rows;
peel one unpacked dimension at a time. Preserve named/default iterator identity,
signed 32-bit declared indices, enclosing reads and restored bindings even on
failure. Reject iterator writes, illegal dimensions, unmapped nonintegral rows or
nonintegral maps. Seed with the first mapped element to preserve singleton Z.
Reverse freezes receiver coordinates once; sort rereads that live receiver at
fixed coordinates for swaps, including singleton-selector side effects.

## Events, predicates and streams

Register event or-lists atomically with `llg_wait_any_events`, never sequential
waits. True waits execute once immediately; false/X/Z constant waits suspend on
empty dependencies without polling or blocking time advancement. Evaluated events
compare expression values, not every operand change; qualifiers run at trigger.
Packed edges use LSB, real any-change uses IEEE bits (signed-zero changes wake;
identical NaN payloads do not). Copy supported automatic evaluator captures and
transitively reject disallowed callback effects/captures. Named/mixed events retain
identity and atomic registration. `->>` captures delay at issue and queues NBA;
event/repeat timing uses an independent detached waiter, whose final trigger is
still NBA. Detached repeat counts remain constant until capture is represented.

Sequential predicates evaluate clauses once, left-to-right, continuing only on
definite true. Preserve explicit branches/pattern bindings and short-circuit
unreachable constants; never replace with `&&`. Matching sources/selectors are
captured once; filters use per-item bindings and first-match/default order.
Tagged pattern checks use the enclosing case mode; ordinary member guards are exact.
Recursive constant checks read each structure member in its own
state domain, including two-state conversion inside a four-state packed record.
Constant-pattern operands admit fixed integral atoms, packed structures and
untagged packed unions; reject unpacked, real and native shapes before scalar
comparison without changing their owned descriptors.
`inside` follows expression result types before storage roots, keeps
casts, descends arrays to singulars (not struct members), evaluates selector and
scalar/range endpoints once, and uses wildcard equality/inclusive comparisons.

Streams retain typed direction, slice size, operand order, unsigned result and
component LHS widths. Snapshot RHS once. Runtime fixed-array `with` selectors are
one-dimensional only, not multidimensional arrays or decorated nested concats;
traverse declared storage order. Each target selector may observe earlier unpacked
fields, unlike frozen positional targets. Wider fixed destinations left-align and
zero-fill; oversize fixed sources diagnose, while longer unpack sources consume
leftmost bits. Composite call/ref layouts use the same recursive descriptors.
Memory views evaluate each dynamic selector once, retaining static strides and HDL
bounds; invalid views fail before writes. Binary rank-one reads advance numeric
addresses independently of declaration order. Packed `$fread` optional bounds
are evaluated/disposed but their values are ignored; memory bounds remain active.

## Time, output and native services

Use owned nearest-module units/precision and typed `IrDelay`; round complete real
delays once locally before checked scaling. Packed X/Z delay becomes zero, negative
packed values convert to unsigned 64-bit time; reject nonfinite/negative real delays
and overflow. `timescale.rs` owns tick representation; keep the runtime timescale-
agnostic. `$time`/`$stime` round to calling units with halves upward; `$realtime`
keeps fractions. `%t` carries physical units through runtime `$timeformat`;
`$printtimescale` uses caller metadata. Frontend owns inheritance.

Real/shortreal use typed doubles and f32 rounding at shortreal writes. Real-to-packed
rounds nearest, ties away; packed-to-real zeroes X/Z positions at actual width.
`$rtoi` truncates to signed 32-bit (nonfinite X, finite overflow modulo 2^32);
`$itor` preserves packed width/sign and rounds real input through signed 32-bit.
IEEE bit-reinterpretation inverses require 64/32 bits and zero X/Z. Typed constants
share conversions. Validate all 21 Table 20-4 math functions, evaluate each argument
once and retain libm domain/nonfinite behavior. Do not infer real expressions from
source or bypass context-specific rejection guards.

Parse display formats during lowering; `%d` retains signedness and `$write` omits
newline. Monitors/strobes run after settled Active/Inactive/NBA work; latest monitor
only, registration/enabling forces a report, time-only arguments do not retrigger.
Deferred callbacks need owned environments. Waveform controls preserve source/depth
identity against the fixed catalog; omit waveform/libfst when unused, reject
`$dumpports`, warn/skip `$displayon`/`$displayoff`. Packed string literals remain
unsigned MSB-first byte vectors (empty is one zero byte), with normal pad/truncate;
SystemVerilog strings remain separate native owners.

Concurrent assertions use dedicated instances, immutable sampled predicates,
explicit clocks/disable/control metadata, per-attempt/thread locals and ordered
match-item effects. Preserve Slang actual/default expansion; reject conflicting
clocks, unsupported temporal/cross-clock forms or unrepresented copy-out. Deferred
assertions retain issue-time values and Reactive actions under the single-call
contract, rejecting unowned automatic/dynamic refs or timing/control actions.
Mailboxes preserve typed copy/identity semantics and delegate waits/cancellation to
runtime queues. Container kinds keep distinct storage, key conversion, notification
and lifetime rules; another container's admitted operation is not a fallback.
