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
owned data and emits typed IR, not C source.

See [the parent README](../readme.md) and
[the detailed source map](../../../../docs/source_layout.md).
