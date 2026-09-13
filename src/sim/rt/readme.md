# `sim/rt`

- **Purpose:** embedded C11 runtime sources compiled into each generated model;
  they are not linked into Rust binaries.
- **Value layer:** `llg_value.h/.c` implements model-width four-state values,
  operations, resolution, formatting, and numeric conversions.
- **Legacy random layer:** `llg_random.h/.c` implements Verilog-2001
  `$random` and the seven `$dist_*` functions using the specified Annex N
  algorithms. It is scheduler-independent and can be compiled as a standalone
  C11 translation unit.
- **Reference layer:** `llg_ref_t` describes a whole packed value or legal
  packed/array selection; `llg_ref_read` and `llg_ref_write` preserve immediate
  alias visibility while routing writes through normal force/PCA notifications.
- **True-net aliases:** generated `llg_net_alias_t` descriptors project each
  aliased bit from its canonical resolved net group into visible storage;
  net publication refreshes dependencies and waveform observations for every
  alias name.
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
  `$swrite`/`$sformat`/`$sformatf` use the same typed formatter as display tasks;
  formatted results own their bytes independently of source arguments.
  Semaphores (§1800-2009 15.3) keep runtime-owned key counts and a specified
  FIFO waiter queue; blocking `get` registrations are removed on process
  cancellation and all semaphore storage is reclaimed at runtime cleanup.
  Hosted C targets are required; freestanding targets are unsupported.
  File output uses an owned 32-slot descriptor table: stdout/stderr masks,
  ordinary host files, multichannel fan-out, typed deferred output, checked
  seek/rewind/flush/error/EOF controls, an HDL-aware formatted scanner, line
  and character pushback, and declaration-order binary reads are kept separate
  from scheduler state. File-input target descriptors are borrowed for one
  call; packed X/Z state and native string ownership remain explicit.
  Clocking input samples use the preponed/observed history services, while
  procedural clocking output/inout drives enqueue captured Re-NBA values after
  their constant output skew; an off-event drive is retained until the next
  matching clocking event, and net drives retain their resolved driver slot.
  Clocking-bound `##N` waits are lowered as repeated event waits, so they count
  resolved clocking edges instead of assuming a clock period.
  Memory-file tasks parse four-state binary/hex words, comments and address
  jumps into bounded fixed packed memories, and write the same consumable
  format in declaration/range order; resizable, multidimensional and real
  memories remain an explicit lowering boundary.
  Deferred immediate assertion actions use an owned per-time-slot report queue:
  conditions and value arguments are sampled at issue time, legal references
  are resolved by the Reactive callback, and same-process assertion identities
  coalesce before the Observed-to-Reactive handoff. Finish/deadlock teardown
  drains pending reports before releasing the scheduler.
  `LLG_ZERO_LOOP_LIMIT` bounds scheduler passes (default 10,000,000), while
  `LLG_PROCESS_STEP_LIMIT` bounds generated loop back-edges inside a coroutine
  (`LLG_NONCONVERGENCE_LIMIT` is an accepted alias). Both accept positive
  decimal `uint64_t` values through the environment;
  invalid or overflowing values fail before model execution. A process budget
  exhaustion emits its process source location and makes the generated model
  exit nonzero. Scheduler ticks are exact femtoseconds; the generated model
  supplies checked local-unit conversions for `$time`, `$stime`, and
  `$realtime`. Fine-grain `process` handles use a reference-counted identity
  separate from coroutine storage; `self`, status, kill, suspend, resume and
  await retain terminal state and clean wait/frame/descendant ownership.
- **VPI bridge:** `llg_vpi.c` and the emitted `vpi_user.h` expose a bounded,
  generation-checked object/iteration/value API, startup-loaded system-task and
  function plugins, compiletf/sizetf/calltf dispatch, and start/end callbacks;
  unsupported standard properties fail through `vpi_chk_error`.
- **Mailboxes:**
  Typed and untyped mailbox handles use owned FIFO message nodes with optional
  bounds (`new(0)` is unbounded), exact `num`/`put`/`get`/`peek` and
  `try_*` operations, native string ownership, four-state packed copies, and
  class/chandle pointer identity. Blocking producers and consumers have FIFO
  wait lists; process cancellation removes waiters and destroys pending
  string messages before mailbox teardown.
- **Random streams:** `llg_rng.h/.c` provides deterministic PCG streams with
  stable process/fork derivation, unbiased inclusive ranges, and versioned
  state snapshots. The scheduler binds one stream to each generated process;
  the standalone service is also suitable for future class-object streams.
- **Concurrent assertions:** Registrations retain per-instance FIFO attempts;
  predicates read immutable Preponed packed snapshots in Observed, asynchronous
  `disable iff` clears pending attempts at writes, and pass/fail actions queue
  in Reactive. Vacuous implication successes are counted separately, while
  pending attempts are discarded at end of simulation. Sequence graphs carry
  bounded local-variable descriptors, per-thread four-state snapshots, local
  input-formal initializers, and ordered match-item callbacks; branch joins
  deduplicate only equivalent local snapshots, so overlapping attempts and
  distinct sequence threads do not share mutable state.
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
- **Checks:** standalone value/random/container runtime probes and Rust
  integration tests cover the runtime boundary.

See [`docs/sim_data_semantics.md`](../../../docs/sim_data_semantics.md) for
language-level value semantics.
