# C emitter

- **Purpose:** render a validated `sim::execution::ExecutionModel` into a
  standalone generated C11 model.
- **Components:** modules render expressions, statements, functions, and whole
  models; `names.rs` owns C identifiers and `error.rs` owns typed failures.
- **Boundary:** whole-model emission depends on explicit execution blocks and
  terminators; detached operation entry points validate against their checked
  typed tables before indexing.
- **Model ABI:** `model.rs` derives packed capacity and emits model-wide C
  definitions used by every generated translation unit.
- **Build handoff:** `build.rs`/CMake combines emitted model sources with the
  embedded runtime.

See [`docs/sim_data_semantics.md`](../../../docs/sim_data_semantics.md) for
width and conversion semantics.
