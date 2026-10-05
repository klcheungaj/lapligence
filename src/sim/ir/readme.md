# Typed executable operations

The IR represents typed storage, expressions, lvalues, and operations. Frontend
meaning and synthesis classification belong to `sim::semantic`; scheduling
ownership belongs to `sim::execution`. Constructors/accessors enforce local
invariants; `IrModel::validate` and detached-node validation check references,
widths, constants, shapes, and registrations before backend indexing. Recursive
validation covers each variant and its cross-table constraints.

`UdpEval` keeps combinational primitive rows in shared `udp_tables`, with typed
0/1/X input masks and output symbols. Validation checks definition references,
row arity, scalar inputs and the unsigned one-bit result. Optimizer and execution
walkers retain every input; row evaluation stays out of expression trees.

## Ownership and source organization

`ExecutionModel::lower` moves staging process bodies into executable blocks.
Staging entries retain names, helpers, spawn/process/program identities, and
write dependencies because functions, storage, and calls share their checked
tables. Concurrent assertions remain in `IrModel::assertions`, with validated
clock/disable sources, sampled predicates, overlap modes, labels, and Reactive
action identities rather than ordinary procedural blocks.

Large designs hold many statement vectors and expression trees at once, and
every slot is sized by its enum's largest variant. Rare or optional payloads
(system functions, lvalue selectors, container/object/call statements and
optional expressions) are boxed; `ir_records_stay_compact` bounds `IrExpr`,
`IrLhs` and `IrStmt`.

`ir.rs` owns `IrModel` and re-exports expression, lvalue, call, statement, event,
assertion, process, function, initialization, storage, and VPI domains. Containers
and objects remain separate. `validate.rs` supplies the shared context for
`validate/` domain checks. See the [source map](../../../docs/source_layout.md).

## Storage, views, and delayed work

Variable aliases identify canonical storage. True-net aliases retain bit-level
resolved-group bindings through pruning, including read dependencies,
force/release descriptors, and waveform observations. Net-array cells retain
checked bounds and electrical target widths.

Fixed formals carry recursive shapes and exact-width payloads. Union sizes use
the largest member; structs/arrays follow declaration order. Defaults preserve
unpacked leaf state domains and explicit member initializers. `StructMux`
retains immediate unpacked-member boundaries and default-uninitialized constants
over a flattened payload; validation requires disjoint complete coverage without
changing packed mux or `ArrayMux` semantics.

Reference bit writes carry typed indices through validation, traversal,
optimization, and stack sizing. Unpacked member/element references can forward
selected views; frontend-illegal packed bit/part reference actuals remain rejected.

`IrInitialization` retains declaration identity, lifetime, origin, execution
phase, and checked persistent targets for static scalar/fixed composites.
`IrInitStep::Execute` carries a static declaration initializer whose typed
transport is a statement (descriptor-backed fixed arrays); it runs in the
before-process initialization frame, must not suspend, and is validated,
traversed and counted as storage reads/writes like process statements.
Automatic initialization stays activation-local; static locals are not lazily
initialized on first call.

Event evaluators, contexts, read dependencies, delayed NBAs, deferred assertion
actions, and real math functions are validated before optimization/emission.
Contexts/actions use activation-owned storage identities, not transient C
addresses. Deferred updates remain distinct from suspension. Procedural delays
hold constant ticks or typed expressions with unit/precision scales, traversed
by validation, effects, optimization, and stack sizing. Cancelable inertial
updates capture packed values and require persistent whole-driver storage.

## Frozen fixed-value, view and projection contract (RTL-099)

The fixed RTL gate freezes these IR shapes; the SIM wave extends them only
additively (new variants, new fields with defaults), never by changing the
meaning of an existing variant:

- Fixed storage: `IrArray` dense cells or descriptor storage (`sparse()`, above
  the 4,096-cell dense threshold), `IrArray::element_uninitialized` defaults,
  and `IrNetArray`/`IrNetCellRun` constant runs for undriven net-array cells.
- Values above packed capacity: `IrFixedValue` (`Array(IrMemoryView)`, `Call`,
  `Conditional` with `element_cells`, `Stream`, `Convert`, and the additive
  stream/pattern operands of RTL-103: `Dense` views of dense integral storage,
  `Packed` values split into cells, `Selected` runtime `with` selections of
  one-dimensional descriptor arrays; the last two may be runtime-sized and
  count as zero bits for static checks), assigned by
  `IrStmt::FixedValueAssign` and compared by `IrExprKind::FixedValueCompare`;
  whole arrays also use `FixedArrayCopy`, `FixedArrayDeclare`,
  `FixedArrayFill`, `FixedArrayOrder` and `FixedArrayReduce`.
- Calls: `IrCallArg::FixedArray`/`FixedValue` and `IrFormal::fixed_array`; a
  descriptor return is a trailing output formal; `ref` passes the view.
- Projections: `IrElemSel::PackedChain` steps and checked selected reference
  views keep every intermediate boundary; writer analysis uses
  `PackedRange`-over-`ArrayContents` cell intervals that never reach the IR.

The matching runtime entry points are the `llg_fixed_array_*` functions in
`rt/llg_rt.h`; value payloads stay behind the `sv4_*` facade. Native, real,
string and resizable payloads are not part of this contract: they keep their
own container/object descriptors and must not be routed through
`IrFixedValue` by widening its element domain.

RTL-101 extends the contract additively with `IrArray::descriptor` (default
`false`): it forces descriptor storage regardless of extent. Column-layout
records and tagged unions (wider than the packed limit, or with a member array
above the dense threshold) store each member array and scalar leaf as such an
array, a scalar leaf as one cell, and a tagged union's tag as one more cell.
Every whole-value operation lowers to the existing per-column shapes
(`FixedValueAssign`, `FixedValueCompare`, `IrCallArg::FixedValue` and trailing
output formals); no IR variant changes meaning. Remaining limits are in
[known issues](../../../docs/known_issues.md#remaining-non-flattened-fixed-value-contexts).

RTL-101b adds one expression variant, `IrExprKind::Sequence`
(`IrSequenceExpr { statements, value }`): run non-suspending setup statements,
then yield `value`. Validation admits only lexical declarations
(`FixedArrayDeclare`, `NativeValueDeclare`), blocking copies (`FixedValueAssign`,
`FixedArrayFill`, `Assign`, `NativeValueCopy`, string/chandle assignments),
function calls and blocks of these. Lexical storage declared there belongs to the
expression's value scope and is released with it. Lowering uses it for a call
returning a column-layout record inside an expression (the call fills a lexical
temporary whose columns the comparison or selection reads) and for a whole-value
pattern binding (the copy into the binding's storage, which the enclosing
statement declares). Effect analysis, storage marking and the optimizer
traversal visit its statements; the owned emitter rejects it in read-only
evaluator callbacks. A subroutine record value with real, string or chandle
members keeps them in one SIM-003 native value (a structure of those leaves
with the record type's nominal identity), passed as one more trailing formal
after the record's columns; no `IrFixedValue` carries a native payload.

## Descriptor-backed native values (SIM-003)

Native records live next to, not inside, the fixed-value contract.
`IrModel::native_types` holds bounded `IrContainerElement` record/fixed-array
shapes whose leaves are packed, real, string or chandle
(`native_values::validate_native_type`: depth and per-level item limits,
nonzero nominal ids, no unions, events, classes or containers).
`IrModel::native_values` names each storage root: persistent roots carry a
unique C name; activation roots are declared lexically by
`IrStmt::NativeValueDeclare` or bound by an `IrFormal::native_value`, and the
validator rejects uses outside that scope. `NativeValueCopy` copies between
roots of one type. Calls pass `IrCallArg::NativeValue` (a root),
`NativeLeaves` (a fresh root built from leaf expressions) or `NativeCall`
(a fresh root filled by a nested native-result call). Leaves are read and
written through `IrNativeAccessKind::ValueItem` accesses with a constant
`item_path` and the existing local/string/chandle statement forms, so effects,
optimization and stack sizing see ordinary named reads and writes.

## Native nonblocking writes and conditionals (SIM-004)

`IrStmt::DelayedStringAssign` and `DelayedChandleAssign` name persistent
string/chandle storage by C name and queue an owned issue-time value; the
untimed `<=` form is the same statement with `IrDelay::Constant(0)`. Module
record NBAs lower to one such statement (or a packed `Assign`/`DelayedAssign`)
per leaf. `IrStringExpr::Conditional` and `IrChandleExpr::Conditional` carry an
integral predicate and two lazily evaluated arms; an ambiguous predicate keeps
equal values and otherwise yields `""` or null. Native record conditionals
lower to ordinary `If` statements over captured leaves, never to
`IrFixedValue`.

## Recursive containers and handle storage (SIM-006)

`IrContainer` storage has three homes. A model-global container
(`is_global_storage`) is one static variable with contents and shape
dependencies. An activation container (`activation`) is created by
`IrContainerStmt::Declare` (subroutine and procedural-block automatic locals,
call temporaries) or bound to a container formal (`IrFormal::container`, a
`void*` parameter naming caller-created storage of the same type); it has no
dependencies, so event controls and monitors on it are rejected. A class
property container (`class_field`) lives in each object and is addressed only
through the receiver of the class's own methods. Calls pass
`IrCallArg::Container` (the call copies inputs in and outputs back, so the
callee never aliases the actual) or `ContainerValues` (a packed/real pattern
built at the call); a container result is a trailing output formal.

Element types may be records, fixed arrays and identity handles (events,
processes, class-like objects, chandles). Whole record elements move through
`IrContainerStmt::SetValue`/`GetValue` with `IrValueSlot` (element, push,
insert, pop) and native temporaries; members are read and written in place
through `IrNativeAccessKind::ElementItem` with a `ContainerElement` receiver,
and writes notify readers once the statement completes. Event elements are
reached through `IrEventRef::Handle`; process elements through
`IrProcessExpr::Handle`. Containers never travel as `IrFixedValue`.

## Real references, real arrays and real sampling (SIM-005)

Reals stay numeric and never enter the integral fixed-value contract.
`IrCallArg::RefAddr` with width 0 binds a real `ref` formal (`IrFormal::real`
plus `Ref` mode) to one real cell: a real signal, real local/formal storage, a
whole real array element, or a forwarded real reference (`IrLhs::Ref` width
0, writable only through a real ref formal). The C ABI is a `double*`; writes
are ordinary real stores. `IrFormal::real_array` names real fixed-array
storage of a formal or result (an `IrArray` with `real`); automatic storage is
an activation declared by `FixedArrayDeclare` and emitted as a lexical
`double` buffer, never `llg_fixed_array_t`. Its operands are
`IrCallArg::RealArray` (whole storage: copied for inputs/outputs, aliased for
`ref`), `RealArrayValues` (declaration-order element values for an input) and
`RealArrayCall` (a nested real-array result, like `NativeCall`).
`IrStmt::RealArrayOrder` reorders stored real cells numerically.
`IrSampledFunc::RealStable/RealChanged` and `$past` over a real keep the
argument's exact 64-bit IEEE image in the history domain and compare or decode
it as a real; `$sampled` of a real reads a numeric Preponed snapshot.
A procedural `$sampled` registers every signal its argument reads: reals
through `IrInitStep::RegisterSampled`, packed signals through
`IrInitStep::RegisterSampledValue`, which keeps the Preponed value without the
per-slot history that clocking input skews read.

## Bounded packed selection chains

`IrElemSel::PackedChain(Vec<IrPackedSelect>)` stores successive fixed-array-element
slices. Each step has a typed integral physical-LSB base relative to the preceding
value and a positive result width including remaining element stride. Negative,
unknown, or out-of-range bases represent X/no-write behavior, not malformed IR.

Validation rejects empty chains, real selectors/elements, zero widths, and read
results not matching the final unsigned width. Nested expressions participate in
reference checks and capacity accounting. Both expression visitors expose every
step to optimization, dependency discovery, address snapshots, and stack sizing.
Folding cannot merge steps or erase intermediate bounds: clipping is relative to
each selected value, even when the root contains more bits.

## Fixed-array reductions

`fixed_array_reductions.rs` models nonempty folds over immediate unpacked elements.
`Array` sources reference rank-one storage; `Value` sources hold declaration-order
payloads. Maps bind an item and signed 32-bit declared index lexically, preserving
enclosing locals/formals. Nested sources evaluate before inner bindings shadow them.

Validation checks extent, element/result widths, map tags, and iterator shapes.
Capacity includes the materialized index; traversal, reads, effects, and stack
accounting cover payload and map expressions. Optimization may simplify children,
not promote destination widths or discard receiver/map effects. Seeding from the
first element preserves singleton Z values. Folds do not imply resizable storage.

## Sequential predicates and patterns

`IrExprKind::Predicate` is a nonempty ordered sequence producing unsigned one-bit
truth without contextual fill. Each reached clause uses its entire value: real
nonzero or packed reduction-OR. Only definite true advances; false/X returns
zero/X without evaluating later clauses. This is not `LogAnd`. Validation,
capacity, effects, traversal, reads, and stack budgets inspect every potential
clause. The IR carries no pattern bindings or source DB references.

`IrExprKind::Pattern` owns its value, optional constant/binding, recursive
structure/tagged checks, and exact/`casez`/`casex` mode. Tag discriminants always
match exactly; wildcard modes affect payload constants only. Pattern cases
capture one selector `LocalRead` shared by all items, with ordered filters.
