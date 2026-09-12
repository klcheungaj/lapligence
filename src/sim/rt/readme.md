# `sim/rt`

- **Purpose:** embedded C11 runtime sources compiled into each generated model;
  they are not linked into Rust binaries.
- **Value layer:** `llg_value.h/.c` implements model-width four-state values,
  operations, resolution, formatting, and numeric conversions.
- **Reference layer:** `llg_ref_t` describes a whole packed value or legal
  packed/array selection; `llg_ref_read` and `llg_ref_write` preserve immediate
  alias visibility while routing writes through normal force/PCA notifications.
- **Simulation layer:** `llg_rt.h/.c` implements typed IEEE event-region
  scheduling, signal/driver updates, process services, simulator system tasks,
  region callback hooks, immutable sampled views, nonreturning `$finish`
  controls, typed runtime severity diagnostics (`$info`, `$warning`, `$error`,
  `$fatal`) with stable counters, resumable `$stop` suspension with explicit
  resume/exit policy, the exactly-once final-block phase, and checked zero-time
  budgets.
  `$system` is a separately gated generated-process host boundary: the child
  must opt in with `LLG_ALLOW_SYSTEM`, and enabled calls return the host C
  `system()` status without normalizing shell or platform behavior. Its omitted
  form preserves `system(NULL)`, distinct from an explicit empty command.
  Hosted C targets are required; freestanding targets are unsupported.
  File output uses an owned 32-slot descriptor table: stdout/stderr masks,
  ordinary host files, multichannel fan-out, typed deferred output, and
  seek/rewind/flush/error/EOF controls are kept separate from scheduler state.
  `LLG_ZERO_LOOP_LIMIT` bounds scheduler passes (default 10,000,000), while
  `LLG_PROCESS_STEP_LIMIT` bounds generated loop back-edges inside a coroutine
  (`LLG_NONCONVERGENCE_LIMIT` is an accepted alias). Both accept positive
  decimal `uint64_t` values through the environment;
  invalid or overflowing values fail before model execution. A process budget
  exhaustion emits its process source location and makes the generated model
  exit nonzero. Scheduler ticks are exact femtoseconds; the generated model
  supplies checked local-unit conversions for `$time`, `$stime`, and
  `$realtime`.
- **Real dependencies:** scalar `real`/`shortreal` storage uses typed double
  dependencies for `wait`, any-change `@` controls, combinational links, and
  scalar ports. Writes notify only when the IEEE representation changes:
  signed-zero transitions wake, identical NaN payloads do not, and changed NaN
  payloads wake deterministically.
- **Evaluated events:** event and trigger-time qualifier callbacks receive an
  owned activation-frame context. The expression wait takes ownership of the
  initial frame references and releases them on wake, cancellation, or runtime
  teardown; callbacks cannot suspend or mutate scheduler-observed storage.
- **Storage helpers:** `llg_string.h/.c` provides owned strings;
  `llg_container.h/.c` provides packed fast-path containers plus descriptor-
  driven dynamic arrays for represented real, string, chandle, and nested
  values. Queues and associative arrays remain packed-only until their typed
  container paths are implemented.
- **Optional components:** `llg_wave.h/.c` provides waveform output with VCD/FST
  headers expressed in the exact femtosecond tick unit; `gtkwave/`
  contains the pinned FST sources; libaco sources provide model coroutines.
- **Embedding:** `mod.rs` exposes source pairs; `sim::build` writes them with
  generated model sources and builds them with CMake.
- **Checks:** standalone value/runtime self-tests and Rust integration tests
  cover the runtime boundary.

See [`docs/sim_data_semantics.md`](../../../docs/sim_data_semantics.md) for
language-level value semantics.
