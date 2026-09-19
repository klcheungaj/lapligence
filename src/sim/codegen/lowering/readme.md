# Lowering domains

- **`lowering.rs`:** owns `Codegen`, orchestration and shared lowering data.
  Context, references, initialization, delay and selection domains live beside
  the facades below.
- **`collection.rs` / `collection/`:** collect design storage, nets, gates,
  ports, subprogram signatures/bodies, calls, processes, dependencies and events.
- **`statements.rs` / `statements/`:** `EmitCtx` coordinates procedural dispatch,
  declarations, assignments, control flow, events, forks, drivers, assertions,
  clocking, system tasks and calls.
- **`expressions.rs` / `expressions/`:** lower typed expressions, operations,
  conversions, aggregates, streaming, membership and system-function queries.
  `expressions/aggregates/copies.rs` separates selected-value type compatibility
  from root storage and pairs fixed-array leaves in declaration order. Source
  leaves are captured before any destination leaf is written.
- **`containers.rs` / `containers/`:** container initialization, indexing,
  queries, fixed-array views, streaming, assignment, methods and callbacks.
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

Storage collection lowers constant declaration initializers eagerly, but a
scalar declaration initializer whose expression contains a user function call is
deferred until subroutine prototypes have assigned every callee a model entry.
Deferred initializers are replayed in the model initialization frame with process
recursion depth zero; a failed replay aborts code generation rather than emitting
a partial model. A default that references an earlier side-effecting actual is
rejected because no caller-side input staging exists yet to evaluate that actual
once.
