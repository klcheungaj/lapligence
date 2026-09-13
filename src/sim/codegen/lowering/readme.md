# Lowering domains

- **`collection.rs`:** collect storage and wiring from the owned database and
  build processes and initialization data.
- **`statements.rs`:** lower procedural statements through `EmitCtx`.
- **`expressions.rs`:** lower expressions and assignment targets to typed IR.
- **`assertions.rs`:** lower the bounded concurrent-assertion sequence
  automaton, named instance expansion, sampled property composition, and
  ordered local match-item effects over per-attempt sequence state.
- **Shared state:** `lowering.rs` owns orchestration, data types, and helpers;
  children communicate through explicit `pub(super)` seams.
- **Boundary:** Slang semantic capture is confined to `core::db`; these modules
  contain no frontend calls, `unsafe` code, or C source emission.

See [the parent lowering README](../readme.md) for the public lowering role.
