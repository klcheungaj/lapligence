# lowering domains

The parent `lowering.rs` module owns shared lowering state, data types, pure
helpers, and the public orchestration entry points. Its child modules group
behavior by phase:

- `collection.rs` walks the owned database, collects storage and wiring,
  resolves functions/tasks, and constructs processes and initialization data.
- `statements.rs` lowers procedural statements through `EmitCtx`.
- `expressions.rs` lowers expression and assignment targets to typed IR.

Children use only the parent module’s shared state and explicit `pub(super)`
method seams. UHDM traversal remains confined to the owned `core::db`; these
modules contain no VPI calls, unsafe code, or C source emission.
