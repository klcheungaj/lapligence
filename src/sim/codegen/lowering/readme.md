# Lowering domains

- **`collection.rs`:** collect storage and wiring from the owned database and
  build processes and initialization data.
- **`statements.rs`:** lower procedural statements through `EmitCtx`.
- **`expressions.rs`:** lower expressions and assignment targets to typed IR.
- **Shared state:** `lowering.rs` owns orchestration, data types, and helpers;
  children communicate through explicit `pub(super)` seams.
- **Boundary:** UHDM traversal is confined to `core::db`; these modules contain
  no VPI calls, `unsafe` code, or C source emission.

See [the parent lowering README](../readme.md) for the public lowering role.
