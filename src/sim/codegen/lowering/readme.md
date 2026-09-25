# Lowering domains

- **`lowering.rs`:** owns `Codegen`, orchestration and shared lowering data.
  Context, references, initialization, delay and selection domains live beside
  the facades below.
- **`collection.rs` / `collection/`:** collect design storage, nets, gates,
  ports, subprogram signatures/bodies, calls, processes, dependencies and events.
  Input value links pre-collect an explicitly bound hierarchical static
  block-local actual into its per-instance storage before lowering the link;
  the same storage supplies the source dependency so later writes refresh the
  child port. Automatic and unresolved actuals remain fail-closed.
  Packed selections of subroutine formals, function return slots, and
  procedural locals lower against their existing typed activation or local
  storage roots; this avoids requiring a module-wide signal for those targets.
  `collection/call_contracts.rs` admits static return storage to read-only event
  callbacks only when it is not read and a result value is established on
  every normal or explicit return path. `collection/return_flow.rs` preserves
  abrupt exits separately and consumes break/continue only at their lexical
  loop. Unmodelled transfers fail the proof rather than reaching later writes.
  Ordinary procedural static calls do not use this private-result substitution.
  A supported static function result
  exposed to a hierarchical assignment or passed as a hierarchical ref actual
  is registered as a hidden model signal backed by the same persistent `_ret`
  storage for continuous/procedural writes and caller references.
- **`statements.rs` / `statements/`:** `EmitCtx` coordinates procedural dispatch,
  declarations, assignments, control flow, events, forks, drivers, assertions,
  clocking, system tasks and calls. Fixed `foreach` nesting uses the owned
  statement's per-slot bounds, including packed element dimensions, rather than
  reconstructing dimensions from storage arrays. Fork branches use persistent
  subroutine signals directly; synchronous `join` branches borrow enclosing
  automatic numeric cells, while detachable branches retain value snapshots.
- **`expressions.rs` / `expressions/`:** lower typed expressions, operations,
  conversions, aggregates, streaming, membership and system-function queries.
  `expressions/aggregates/copies.rs` separates selected-value type compatibility
  from root storage and pairs fixed-array leaves in declaration order. Source
  leaves are captured before any destination leaf is written.
- **`containers.rs` / `containers/`:** container initialization, indexing,
  queries, fixed-array views, streaming, assignment, methods and callbacks.
  `containers/fixed_arrays.rs` keeps specialized array conversions and a
  type-directed value fallback. Its fixed-value source helper stages a complete
  RHS once before projecting destination cells; runtime selected views capture
  their selectors once before projecting source cells. Module-procedural
  expressions and fixed-array input value ports reuse the same typed conditional
  operation as subroutine values. Assignment-pattern consumers use Slang's
  resolved declaration-order operands for recursive type-key/default patterns;
  explicit index keys are evaluated from owned constant values or expression
  nodes. Positional pattern lvalues recursively scatter fixed unpacked arrays,
  structures, and packed arrays after one RHS snapshot. Bound references to
  subroutine-local scalar parameters materialize their captured values on demand
  instead of adding them to instance-wide parameter storage.
- **`objects.rs` / `objects/`:** non-integral class/interface, mailbox, process,
  enum, handle and string operations.
- **`assertions.rs`:** bounded concurrent-assertion sequence automata, legal
  multiclock `##0`/`##1` flow, default-clock inheritance, named instance
  expansion, sampled composition, conditional/abort controls and ordered
  local match-item effects over per-attempt sequence state.

Children use narrow visibility within the existing owner rather than exposing
new public state. Slang capture remains confined to `core::db`; lowering uses
owned data and emits typed IR, not C source. Legacy `Verbatim` nodes may still be
constructed by fenced compatibility paths; the structured owned emitter rejects
them rather than treating embedded C text as an ownership-safe result.

See [the parent README](../readme.md) and
[the detailed source map](../../../../docs/source_layout.md).

Bound numeric arguments and inout copy-in produce converted typed `IrExpr`
values only. Defaults resolve earlier formals through the typed argument map.
Do not request detached C strings in argument binding: owner setup/cleanup is
emitted later by the structured whole-model renderer.

Storage collection lowers declaration initializers eagerly when their references
are available. It defers a scalar initializer that contains a user function call
or names a bound variable/net whose child or interface signal is not collected
yet; other initializers keep their eager order. Deferred
initializers are replayed after design collection and subroutine prototype
emission in the model initialization frame with process recursion depth zero. A
failed replay aborts code generation rather than emitting a partial model. A
default that references an earlier side-effecting actual is rejected because no
caller-side input staging exists yet to evaluate that actual once.

## Fixed-array method maps

`containers/fixed_reductions.rs` consumes owned receiver type descriptors and
peels one unpacked dimension at a time. It retains immediate row/record types,
not just packed leaves, and uses captured iterator declaration IDs to scope
nested maps. Bindings are restored after successful and failed recursive
lowering. Read-only iterator projections share the fixed activation projection
machinery; they are never writable reference destinations.

Whole rank-one model arrays remain ordinal cell sources. Casts, assignments,
selected rows, slices, activation values and function results use expression
sources, evaluated once by the backend. Result widths/signs come from the
integral element or `with` expression, not the destination. This path does not
introduce generated callbacks or change resizable-container callback contracts.

## Sequential conditional predicates

`expressions/conditionals.rs` lowers a Boolean `ConditionalPredicate` to one
`Predicate` IR expression, or reuses the ordinary Boolean conversion for a
single clause. Primitive, fixed-structure and finite tagged-union patterns
lower to owned `Pattern` IR expressions with definite-match binding stores;
tag discriminants use exact four-state equality before payload checks. Pattern-
case statements capture their selector once, then lower
each ordered pattern/filter/body item to the same pattern machinery and a
first-match branch chain. Statement dispatch
uses owned true/false branch references instead of child-vector positions. Value
conditionals share ordinary packed/real arm typing, R01's `ArrayMux` layout,
and SYN-004's immediate-member `StructMux` layout;
R04 array-assignment staging remains separate. All reached clauses stay inside
the resulting expression so the emitter, not lowering, controls their execution.
Constant evaluation stops at the first false or ambiguous clause as well.

## Memory-task views

`statements/system_tasks.rs` lowers fixed packed `$readmem*`/`$writemem*`
destinations into an owned memory view. Higher-dimension runtime selectors are
retained as typed expressions and evaluated once at the task call; a constant
slice may narrow the lowest specified dimension. The view dimension follows
the source array's declared direction, while file addresses continue to map to
their numeric indices. The view retains physical flat-array strides and checks
file start/finish addresses against the selected range. Dynamic, native and
associative memories remain outside this path.
