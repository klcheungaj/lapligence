# `sim/rt`

- **Purpose:** embedded C11 runtime sources compiled into each generated model;
  they are not linked into Rust binaries.
- **Value layer:** `llg_value.h/.c` implements model-width four-state values,
  operations, resolution, formatting, and numeric conversions.
- **Simulation layer:** `llg_rt.h/.c` implements scheduling, signal/driver
  updates, process services, and simulator system tasks.
- **Storage helpers:** `llg_string.h/.c` provides owned strings;
  `llg_container.h/.c` provides dynamic arrays, queues, and associative arrays.
- **Optional components:** `llg_wave.h/.c` provides waveform output; `gtkwave/`
  contains the pinned FST sources; libaco sources provide model coroutines.
- **Embedding:** `mod.rs` exposes source pairs; `sim::build` writes them with
  generated model sources and builds them with CMake.
- **Checks:** standalone value/runtime self-tests and Rust integration tests
  cover the runtime boundary.
- **Coroutine stacks:** local libaco patches preserve ASan shadow state across
  shared-stack switches and avoid reserving swap for unused Linux stack
  headroom. Stack guards and generated stack budgets remain active.

See [`docs/sim_data_semantics.md`](../../../docs/sim_data_semantics.md) for
language-level value semantics.
