# Simulator module

- **Purpose:** compile an elaborated design into a standalone C11 simulator.
- **Pipeline:**
  - `codegen.rs` exposes lowering from the owned `core::db` design to typed IR.
  - `opt.rs` applies conservative IR transformations.
  - `emit_c.rs` renders validated IR as C11.
  - `build.rs` builds the generated model with CMake, the embedded runtime, and
    libaco sources from `rt/`.
- **Runtime:** `rt/` supplies value operations, scheduling, strings, containers,
  optional waveforms, and coroutine support. It is compiled with each model and
  is not linked into the Rust binaries.
- **Real scheduling:** Scalar `real`/`shortreal` writes use typed double
  dependency identities, so legal wait, event, combinational, and port paths
  observe changed values without packed-vector coercion. Signed-zero and NaN
  change behavior follows the runtime's documented bitwise IEEE policy.
- **Boundaries:** lowering reads the owned database; the emitter depends only on
  `sim::execution`; simulator code contains no `unsafe` or direct frontend access.
- **Coverage:** semantic lowering first walks the owned elaborated graph and
  rejects reachable executable nodes without a lowering contract, retaining
  source spans while distinguishing declarations and elaboration-only records.
- **Process families:** `always`, `always_comb`, `always_latch`, and `always_ff`
  retain their typed kind and write dependencies through IR; implicit
  sensitivity and single-writer, timing, event, and assignment contracts are
  checked before C emission.
- **Subroutine aliases:** `ref` and `const ref` formals retain typed modes and
  bind directly to caller lvalues, including legal packed selects and fixed
  array elements; writable aliases commit through the canonical runtime target.
- **True-net aliases:** Legal packed `alias` declarations share bit-level
  resolved driver groups across whole, selected, and concatenated net names;
  force/release, packed port links, dependency wakeups, and waveform reads use
  those groups. Dynamic selects, aggregate, and switch-level alias forms remain
  unsupported.
- **Entry point:** `src/bin/llg.rs` drives compile → lower → optimize → emit →
  build → run.
- **Validation:** simulator behavior is covered by `tests/sim_*.rs`, scheduler
  behavior by `tests/region_conformance.rs`, optimizer equivalence by
  `tests/sim_opt_differential.rs`, and emitter decoupling by
  `tests/emit_decoupling.rs`.

See [`docs/sim_features.md`](../../docs/sim_features.md) for the simulator
support matrix.
