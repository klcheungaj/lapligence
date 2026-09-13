# `sim/rt`

- **Purpose:** embedded C11 runtime sources compiled into cached static archives
  for generated models; they are not linked into Rust binaries.
- **Value layer:** `llg_value.h/.c` implements model-width four-state values,
  operations, resolution, formatting, and numeric conversions.
- **Legacy random layer:** `llg_random.h/.c` implements Verilog-2001
  `$random` and the seven `$dist_*` functions using the specified Annex N
  algorithms. It is scheduler-independent and can be compiled as a standalone
  C11 translation unit.
- **Reference layer:** `llg_ref_t` describes a whole packed value or legal
  packed/array selection; `llg_ref_read` and `llg_ref_write` preserve immediate
  alias visibility while routing writes through normal force/PCA notifications.
  `llg_ref_write_bit` updates one bit through that same descriptor; unknown or
  out-of-range indices leave the value unchanged.
  Packed queue refs retain shared element cells: removals detach a snapshot,
  while surviving refs follow element identities through shifts and reorders.
  Generated call scopes release pins on return; cancellation/teardown unwinds
  the remaining scopes. Detached writes do not notify or modify the queue.
- **True-net aliases:** generated `llg_net_alias_t` descriptors project each
  aliased bit from its canonical resolved net group into visible storage;
  driver/force commits and delayed net-publication commits refresh dependencies
  and waveform observations for every alias name. Reading an alias is pure;
  postponed observers never refresh storage as a side effect of a read.
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
  cancellation. After a live cancellation batch, newly satisfiable FIFO
  heads are granted existing keys without requiring another `put`; teardown
  only removes waiters and never grants new requests. All semaphore storage
  is reclaimed at runtime cleanup.
  Hosted C targets are required; freestanding targets are unsupported.
  File output keeps separate ordinary-FD and MCD banks. Ordinary FDs carry
  bit 31; the three preopened FDs name stdin/stdout/stderr. MCD bit 0 names
  stdout and bits 1..30 name reusable output channels. Only MCDs fan out.
  Closing a channel cancels its pending deferred output before slot reuse.
  Ordinary host files, typed deferred output, checked
  seek/rewind/flush/error/EOF controls, an HDL-aware formatted scanner, line
  and character pushback, and declaration-order binary reads are kept separate
  from scheduler state. File-input target descriptors are borrowed for one
  call; packed X/Z state and native string ownership remain explicit.
  Numeric scanning stops at the conversion-specific prefix and leaves a
  delimiter unread; suppression skips storage but still validates conversion.
  Clocking input samples use the preponed/observed history services. Named
  clocking-block events are published in Observed after all block samples, while
  procedural clocking output/inout drives enqueue captured Re-NBA values after
  their constant output skew; an off-event drive is retained until the next
  matching clocking event, and net drives retain their resolved driver slot.
  Clocking-bound `##N` waits are lowered as repeated event waits, so they count
  published clocking-block events instead of assuming a clock period.
  Memory-file tasks parse four-state binary/hex words, comments and address
  jumps into bounded fixed packed memories, and write the same consumable
  format in declaration/range order; resizable, multidimensional and real
  memories remain an explicit lowering boundary.
  Deferred immediate assertion actions use an owned per-time-slot report queue:
  conditions and value arguments are sampled at issue time, legal references
  are resolved by the Reactive callback, and same-process assertion identities
  coalesce before the Observed-to-Reactive handoff. Off prevents new checks
  without stopping existing concurrent attempts or flushing deferred reports;
  kill cancels attempts, reports and queued actions and disables new checks.
  Finish/deadlock teardown
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
  Array metadata distinguishes packed element widths from real elements, which
  use zero packed-bit width; both remain discoverable in the object catalog.
  `vpi_get_value(vpiVectorVal)` returns simulator-owned scratch storage, valid
  until the next value query or shutdown; the caller supplies no vector buffer.
  Call/argument handles and argument iterators borrow one callback's call
  record. They are tagged with its owner and invalidated at every `compiletf`,
  `sizetf`, and `calltf` exit before the borrowed storage can be released.
- **Mailboxes:**
  Typed and untyped mailbox handles use owned FIFO message nodes with optional
  bounds (`new(0)` is unbounded), exact `num`/`put`/`get`/`peek` and
  `try_*` operations, native string ownership, four-state packed copies, and
  class/chandle pointer identity. Blocking producers and consumers have FIFO
  wait lists; process cancellation removes waiters and destroys pending
  string messages before mailbox teardown. Retrieval distinguishes empty from
  mismatch: try-get/peek return -1 on mismatch without consuming or assigning;
  blocking mismatch reports an error and stops the current run. Waiter service
  only considers FIFO heads. Packed width, sign and state domain, and
  real/shortreal are checked. Each admitted message and destination also retain
  the declared nominal enum/handle type identity, independent of the handle's
  dynamic value (including null). Typedef aliases share identity; equivalent
  virtual-interface types are interned at the owned frontend boundary. Packed
  ref destinations use the reference write operation, not a value-pointer cast.
- **Program origins:** the spawn ABI carries a stable elaborated instance ID
  and an initial-procedure flag. Only initials are counted; their descendants
  inherit the origin without extending program lifetime. Last-initial completion
  cancels remaining descendants of that origin, and all-program completion is
  immediate. `$exit` consults the executing thread's origin, not the lexical
  scope of a called task. A non-program origin returns without terminating.
- **Random streams:** `llg_rng.h/.c` provides deterministic PCG streams with
  next-parent-draw dynamic child seeding, unbiased inclusive ranges, and
  versioned state snapshots. Creating a child consumes exactly one parent draw;
  draws in an already-created child never perturb its parent or siblings.
  Per-instance/package initialization RNGs and full class-object RNG ownership
  still require integration; the dynamic-child fix alone does not close H07.
- **Concurrent assertions:** Registrations retain per-instance FIFO attempts;
  predicates read immutable Preponed packed snapshots in Observed, asynchronous
  `disable iff` and abort controls clear pending attempts at writes, and
  pass/fail actions queue in Reactive. Failed-attempt accounting is separate
  from severity reporting: explicit failure actions (including `else ;`) replace
  the default, and an absent failure action reports the default error in
  Reactive. Vacuous implication successes are
  counted separately, while pending attempts are discarded at end of
  simulation. Sequence graphs carry bounded local-variable descriptors,
  per-thread four-state snapshots, local input-formal initializers, ordered
  match-item callbacks, and owned per-transition clock/edge descriptors for
  legal multiclock `##0`/`##1` boundaries; branch joins deduplicate only
  equivalent local snapshots, so overlapping attempts and distinct sequence
  threads do not share mutable state. Empty alternatives are represented
  separately and concatenations are normalized before emission. Repetition gaps
  explicitly require a false operand; first-match scopes cancel only their own
  invocation's later alternatives while retaining tied endpoints. Multiclock
  boundaries use endpoint physical time and current-slot clock history, not
  callback order. Accepted antecedent endpoints carry owned local snapshots
  keyed by declaration identity into each consequent; inherited locals are not
  reinitialized. All tokens, scope records and snapshots are reclaimed on
  termination, disable or teardown.
- **Packed dependencies:** longest-static-prefix intervals survive lowering and
  writer checks. Typed waits compare the selected bits, including packed slices
  behind fixed-array change markers; unrelated bit updates do not wake them.
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
  generated model sources, caches compatible runtime archives, and builds with
  CMake. Generated source-only projects remain self-contained.
- **Checks:** standalone value/random/container runtime probes and Rust
  integration tests cover the runtime boundary.

See [`docs/sim_data_semantics.md`](../../../docs/sim_data_semantics.md) for
language-level value semantics.

## Source organization

`llg_rt.c` is an ordered facade over `llg_rt_prelude.c` and `scheduler/*.c`;
`llg_container.c` similarly owns `llg_container_prelude.c` and `container/*.c`.
These are private fragments, not independent translation units. This keeps
private state and declaration order with their existing owner.

`mod.rs` concatenates each ordered fragment list into the original flat C
implementation returned by `runtime_sources()` or `container_sources()`.
Generated builds retain the existing filenames and CMake source lists. Keep
both orders synchronized and do not compile the fragments separately.
`tests.rs` checks embedding order against the source facades.

See [the source map](../../../docs/source_layout.md).
