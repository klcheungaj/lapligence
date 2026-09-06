# C emitter

- **Purpose:** render validated `sim::ir` into a standalone generated C11
  model.
- **Components:** modules render expressions, statements, functions, and whole
  models; `names.rs` owns C identifiers and `error.rs` owns typed failures.
- **Boundary:** the emitter depends only on `sim::ir`; detached-node entry
  points validate against the supplied model before indexing tables.
- **Model ABI:** `model.rs` derives packed capacity and emits model-wide C
  definitions used by every generated translation unit.
- **Build handoff:** `build.rs`/CMake combines emitted model sources with the
  embedded runtime.

See [`docs/sim_data_semantics.md`](../../../docs/sim_data_semantics.md) for
width and conversion semantics.
