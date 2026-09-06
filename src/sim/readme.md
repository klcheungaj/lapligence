# Simulator module

- **Purpose:** compile an elaborated design into a standalone C11 simulator.
- **Pipeline:**
  - `codegen.rs` lowers the owned `core::db` design to typed IR.
  - `opt.rs` applies conservative IR transformations.
  - `emit_c.rs` renders validated IR as C11.
  - `build.rs` builds the generated model with CMake, the embedded runtime, and
    libaco sources from `rt/`.
- **Runtime:** `rt/` supplies value operations, scheduling, strings, containers,
  optional waveforms, and coroutine support. It is compiled with each model and
  is not linked into the Rust binaries.
- **Boundaries:** lowering reads the owned database; the emitter depends only on
  `sim::ir`; simulator code contains no `unsafe` or direct VPI access.
- **Entry point:** `src/bin/llg.rs` drives compile → lower → optimize → emit →
  build → run.
- **Validation:** simulator behavior is covered by `tests/sim_*.rs`, scheduler
  behavior by `tests/region_conformance.rs`, optimizer equivalence by
  `tests/sim_opt_differential.rs`, and emitter decoupling by
  `tests/emit_decoupling.rs`.

See [`docs/sim_features.md`](../../docs/sim_features.md) for the simulator
support matrix.
