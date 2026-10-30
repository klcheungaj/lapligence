# Lowering: events, predicates, aggregates and streams

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
identity and atomic registration. An instance `event` class property
(`IrClassField::event`) is a handle slot of its object: `EventTarget::access`
keeps the selecting expression, operations read the slot as an
`IrEventRef::Handle` when issued, and assignment stores the source's object
identity into the slot; a static one is an ordinary model event. `->>` captures delay at issue and queues NBA;
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
strings, chandles, class handles and virtual interfaces: `collection/block_natives.rs`
module docs; they resolve only by declaration identity. Native record leaves without a declaration
initializer take their type's member defaults (`collection/record_defaults.rs`;
the outermost default on a path wins); a module record initializer that is
not a leaf-wise pattern is one record assignment in the static schedule.
String object values (`IrObject::initial`) join that schedule at their
declaration's slot. Native record `ref` formals:
`native_values/record_refs.rs` module docs; specializations are keyed with
`task_specialization` and install an `unpacked_aggregates` alias while the body
lowers. Element, run-time-indexed
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
(`lower_call_writeback`, selectors evaluated at the copy-out); expression calls and
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
