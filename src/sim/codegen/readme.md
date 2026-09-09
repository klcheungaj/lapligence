# Simulator lowering

- **Purpose:** lower the validated `sim::semantic::SemanticModel` into typed
  executable operations and `sim::execution::ExecutionModel` scheduling.
- **Facade:** `codegen.rs` exposes the public generation API and typed
  `CodegenError` failures.
- **Implementation:** `lowering/` coordinates collection, statement lowering,
  expression lowering, and shared state; `timescale.rs` converts
  Slang-resolved module time scales and typed delay values into scheduler ticks.
- **Boundary:** this layer performs no frontend API access, `unsafe`
  operations, or C source emission. The C backend consumes only the resulting
  execution model.
- **Reuse:** `generate_from_db_with_opts` supports multiple optimization
  variants from one owned database.

See [`docs/sim_features.md`](../../../docs/sim_features.md) for the supported
feature surface and rejection boundaries.
