# Simulator lowering

- **Purpose:** lower the owned `core::db` design into validated typed IR.
- **Facade:** `codegen.rs` exposes the public generation API and typed
  `CodegenError` failures.
- **Implementation:** `lowering/` coordinates collection, statement lowering,
  expression lowering, and shared state; `timescale.rs` handles source
  timescales and delay values.
- **Boundary:** this layer performs no direct VPI access, `unsafe` operations,
  or C source emission. The C backend consumes the resulting IR.
- **Reuse:** `generate_from_db_with_opts` supports multiple optimization
  variants from one owned database.

See [`docs/sim_features.md`](../../../docs/sim_features.md) for the supported
feature surface and rejection boundaries.
