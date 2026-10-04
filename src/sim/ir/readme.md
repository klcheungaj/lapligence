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
