# sim::rt — embedded C11 simulation runtime

## Migration boundary

Runtime values are unique owners. Whole-model rendering uses the structured
numeric emitter and rejects unmigrated feature families; legacy fragment APIs
remain fenced. Runtime/wave selftests retain their original assertions with
explicit owner cleanup; keep them active. Do not bypass a feature guard or link
stale generated C. The ABI marker is `LLG_VALUE_ABI_VERSION` (currently 3),
not a model width. Use `tests/runtime_value_storage` for component checks;
Rust-emitted model integration, full parity and native-platform gates remain.
See [coverage](../../../docs/sim_features.md#dynamic-value-migration-acceptance-boundary) and [ownership](value/ownership.md).

## Purpose

Compile the C11 runtime with generated `model.c` into a standalone executable; **never link it
into Rust binaries**:

- `llg_value.h` / `llg_value.c` — scheduler-independent data types, operations, formatting, and
  numeric conversions, compiled as a standalone C11 translation unit with only standard C/math
  dependencies:
  - `sv4_t` — uniquely owned, exact-width storage bounded by the exclusive
    `LLG_SUPPORTED_WIDTH_LIMIT` (`1 << 20`). One allocation contains three 64-bit
    limb planes (`bits`/`x`/`z`); width zero allocates nothing. X/Z remain distinct,
    with Z treated as X by unknown-propagating operations. Every value-returning
    operation returns an independent owner; packed input parameters are borrowed
    unless explicitly documented otherwise. Initialize owners with `SV4_EMPTY`,
    then use clone/copy/move/replace/destroy instead of retaining struct copies.
    Destroy only the allocation addressed by `bits`. Interior pointers expire on
    replacement, movement, or destruction. See [ownership](value/ownership.md).
    Containing runtime objects destroy their fields; coroutine-live owners use
    registered `llg_value_scope_t` scopes, unwound on completion/cancellation.
    No compiler cleanup attributes, VLAs, alloca, or C++ destructors are permitted.
    The old fixed-array and separate snapshot representations are removed.
  - Value ops — arithmetic/logic/reduction/compare/wildcard-equality/casez/casex, mux, concat,
    declaration-ordered enum navigation, repeat, part/bit/indexed-part selects,
    resize/fill/clog2, format and decimal conversion; partially out-of-range part-select reads
    retain valid bits and fill missing positions with X. Semantics mirror `core::elab::Value`
    (kept in sync). `sv4_resolve` combines equal-strength driver values independently of the
    scheduler; `sv4_resolve_strengths` applies per-driver 0/1 endpoints to every canonical net
    mode. An X contribution represents both endpoint ranges, so a known driver resolves the bit
    only when it strictly dominates every possible opposite value. Wire conflicts yield X,
    wired-AND 0 dominates equal-strength X/1 ties, wired-OR 1 dominates equal-strength X/0 ties,
    and Z is neutral in every mode. No explicit or implicit source yields Z. Tri0/tri1 add pull
    defaults and supply0/supply1 add supply defaults at their standard strengths. Bit-vector
    queries count known one bits across all limbs, ignore X/Z for
    `$countones`/`$onehot`/`$onehot0`, and detect either state for `$isunknown`.
  - Real-number hooks — packed-to-`double` conversion across all `sv4_t` limbs (X/Z bit
    positions contribute zero), rounded `double`-to-packed conversion (targets below the supported
    width limit), scalar truth conversion, wide division/modulo/power, and `%f`/`%e`/`%g` formatting
    support the procedural scalar real/shortreal contract. `shortreal` precision is enforced by
    codegen at assignments and initialization; typed double dependencies drive real `wait`,
    any-change `@` controls, scalar real ports, and combinational sensitivity without routing
    through packed storage. Unsupported arrays, continuous assignments, and subprogram storage
    remain rejected before generated C is compiled. `$rtoi` truncates rather than using
    assignment rounding; real/shortreal bitcasts use `memcpy` and require 64-bit `double`/32-bit
    `float` storage. Delay conversion accepts explicit unit/precision tick scales: packed X/Z
    maps to zero, negative packed values convert to unsigned 64-bit time, and real values round
    once at local precision. Scaling overflow and nonfinite or negative real delays produce
    fatal diagnostics before scheduling. See the [lowering guide](../codegen/AGENTS.md) for
    conversion bounds.
- `llg_random.h` / `llg_random.c` — scheduler-independent Verilog-2001 `$random` and `$dist_*`
  functions. The seed update uses explicit modulo- 2^32 unsigned arithmetic, and the
  distribution wrappers clamp checked integer conversions rather than relying on a host `long`
  width. The implementation follows the Annex N reference algorithms and is compiled both with
  generated models and by the standalone runtime test.
- `llg_rt.h` / `llg_rt.c` — event scheduler and simulation-facing services. The header includes
  `llg_value.h`; link the independent value translation unit, never include its implementation.
  Scheduler-private domains live in `scheduler/`. Libaco coroutines execute an IEEE 1800 §4
  region loop with typed queues for Preponed, Active, Inactive, Pre-NBA/NBA/Post-NBA,
  Pre-Observed/Observed/ Post-Observed, Reactive/Re-Inactive/Pre-Re-NBA/Re-NBA/Post-Re-NBA and
  Pre-Postponed/Postponed, plus every PLI control point. The design and reactive sets iterate to
  a fixed point before postponed output. Services include edge/event/level waits, fork/join,
  blocking/nonblocking packed and double assignments, typed packed/real dependency
  notifications, nonblocking named-event NBA triggers, net resolution, force/release, region
  callbacks, sampled-value views, and `$display`/`$monitor`/`$strobe`/`$finish`/`$exit`/`$time`.
  Program processes are launched in Reactive with an elaborated program-instance origin. Count
  initial procedures separately from fork descendants: last-initial completion cancels that
  origin's detached descendants, and all-program completion ends the simulation immediately.
  `$exit` cancels only its thread's originating program; calls without a program-initial origin
  return without terminating. Generated loop back-edges call a cooperative budget point so a
  coroutine that never yields cannot monopolize the host; the diagnostic retains the process
  source location. Completed fork parents remain alive until detached descendants finish;
  process-table slots are reused, and allocations are released on scheduler exit or
  reinitialization.
- `llg_string.h` / `llg_string.c` — owned byte strings. Reads clone storage; expression
  operations consume their arguments; mutations replace or update the target allocation.
  Persistent generated strings retain an optional typed change callback and stable dependency
  marker; unchanged writes do not invoke it, and owned expression clones carry no callback.
  Strings exclude NUL bytes and use ASCII case conversion. `atoreal` parses a decimal prefix
  without admitting C hexadecimal/NaN/infinity spellings; `realtoa` emits enough decimal digits
  for finite-double round trips. Both preserve the existing clone/consume contract.
- `llg_container.h` / `llg_container.c` — scheduler-independent storage for dynamic arrays,
  queues, and associative arrays whose packed elements are full-width `sv4_t` values. Dynamic
  resize preserves the common prefix and default-fills growth; bounded queues apply the LRM
  discard rules; associative arrays keep ordered integral or byte-string keys. All three
  container kinds provide packed-element reduction folds. Every object requires matching
  init/destroy calls. Allocation overflow and exhaustion are fatal diagnostics, while invalid
  indices use default-read/no-op-write method semantics. Integral associative keys containing
  X/Z are rejected before declared-key casting.
- `llg_rt_selftest.c` — C self-tests: sv4 value vectors (mirrored from the `core::elab` unit
  tests) plus scheduler checks (delay ordering, NBA visibility, ping-pong, directly observed
  nested `join_none` lifetimes, empty fork-group finalization, and cumulative process slot
  reuse). Compiled and run by the `sim_rt_selftest` case in `tests/sim_counter.rs`.
- `llg_wave.h` / `llg_wave.c` — optional VCD/FST waveform runtime. The OS simulation thread
  alone produces into a bounded SPSC ring; one POSIX/Win32 writer thread consumes it and owns
  the file. Events own their values, publication uses release/acquire ordering, full and empty
  waits use condition variables, `$dumpflush` is an acknowledged FIFO barrier, and close is an
  in-band event followed by join. Never add a second producer without replacing this contract or
  assigning it a separate SPSC ring. Registration names use ASCII unit separator (`0x1f`)
  between hierarchy components; it cannot occur in a source identifier. The writer reversibly
  encodes punctuation per component, preventing escaped dots from becoming scopes and preventing
  serialized-name collisions.
- `gtkwave/` — pinned GTKWave libfst writer/reader plus its FastLZ/LZ4 support and
  license/provenance files. It is emitted and compiled only for models containing waveform
  controls. FST compression uses zlib and libfst's own internal parallel mode stays disabled
  because `llg_wave.c` owns threading.
- `llg_wave_selftest.c` — forces ring wrap/backpressure, checks the VCD flush barrier, writes
  both formats, and reopens the FST with the official reader.

## Scheduling and value details

The value layout above uses `uint64_t bits[]`, `x[]`, `z[]`, `uint32_t width`, and `int8_t
is_signed`. Keep `core::elab::Value` parity through the property/vector checks in
[../../../tests/AGENTS.md](../../../tests/AGENTS.md).

One coroutine runs each always/initial (including generated scopes), continuous assignment and
port link; ordinary fork branches use `llg_fork`, while captured branches use
`llg_fork_with_frame` with ref-counted activation storage. The creator releases its frame after
spawning, and completion, cancellation, and process teardown release the child-owned frame.
Active coroutines are FIFO. Immediate NBA lists and the global timed NBA queue commit in issue
order within the NBA region, re-iterating to quiescence before advancing time. Future NBAs own
captured values and persistent destination pointers, remain scheduled after process completion,
and can advance time without a timed process waiter. Masked NBA writes merge only selected bits
into current storage. Inertial drivers own one pending active-region update per site and remain
live after their evaluation process ends. A changed pending value cancels its event; an
unchanged value retains its deadline; returning to the current contribution cancels without
replacement. The sorted event queue advances time independently of process waiters. Static
generated handles are reset by cleanup before their runtime-owned driver storage is freed;
reinitialization releases pending events. Zero-delay driver updates drain in the active region.
Strobe and monitor checks run only after active/inactive/NBA work has reached quiescence;
monitor registration and enabling force one queued report, while only registered signal
dependencies mark later checks dirty. The scheduler stops on `$finish` or deadlock. `$stop` is a
separate resumable state: the issuing coroutine yields without being completed, all
same-time/future queues and activation frames remain owned by the runtime, and `llg_rt_resume`
requeues its continuation at the same simulation time. The default `LLG_STOP_POLICY=resume`
(also exposed by `llg --stop-policy resume`) automatically applies that hook so batch runs
cannot wait for stdin; `LLG_STOP_POLICY=exit` / `--stop-policy exit` returns from `llg_rt_run`
with a live suspended context for an embedding to inspect or resume. Final blocks are never run
while the context is suspended, and generated CLI models clean up after an explicit exit policy.
Stop verbosity uses validated levels 0/1/2 and is diagnostic-only. Per-waiter snapshots detect
posedge 0→1, 0→X/Z, X/Z→1 (negedge mirrored), using only the LSB for packed vector edges. Real
any-change snapshots compare the IEEE-754 representation bit-for-bit: signed-zero transitions
wake, identical NaN payloads do not, and a changed NaN payload wakes deterministically.
Expression waits own copied typed dependency lists and value snapshots; `iff` callbacks run at
the trigger, including named events. Wakeup, disable and teardown free these allocations and
unregister all named events. A zero-dependency signal wait remains suspended without polling.
`llg_wait_any` uses snapshots; event or-lists require atomic `llg_wait_any_events`, never
sequential waits. See the lowering guide for force/release and inout resolution boundaries. Net
groups select wire/wired-AND/wired-OR/pull/supply resolution through the standalone value API;
`llg_net_write` publishes only resolved-value changes to waiters. Generated driver cells start
at Z. Lowering explicitly writes X into an existing delayed continuous-driver slot before
processes start, distinguishing that pending driver from a genuinely driverless Z net. Selected
continuous drivers publish a fresh Z-based contribution on every evaluation, so only the
selected range contributes; lowering rejects dynamic net selectors.

## Embedding

- `mod.rs` embeds sources with `include_str!`. `llg_value.c`, `llg_rt.c`, and
  `llg_container.c` include ordered private fragments (`value/`, `scheduler/`,
  `container/`, and their root preludes); `concat!` assembles
  that identical order into flat emitted C. Compile only the facades, never fragments
  independently; keep the facade and embedding orders synchronized. No shared-state export or
  runtime ABI changes.
- Public source-return APIs and dependencies remain:
  - `value_sources()` → `(llg_value.h, llg_value.c)`;
  - `random_sources()` → `(llg_random.h, llg_random.c)`;
  - `rng_sources()` → `(llg_rng.h, llg_rng.c)`, the scheduler-independent process/object stream
    service used by generated random facilities;
  - `runtime_sources()` → `(llg_rt.h, llg_rt.c)`, requiring the value pair;
  - `string_sources()` → `(llg_string.h, llg_string.c)`, requiring the value pair;
  - `container_sources()` → `(llg_container.h, llg_container.c)`, requiring the value pair but
    not the scheduler;
  - `libaco_sources()` → `(aco.h, aco.c, acosw.S)` from `vendor/libaco`;
  - `selftest_source()` → `llg_rt_selftest.c`.
  - `waveform_sources()` / `waveform_selftest_source()` → the optional waveform runtime, libfst
    snapshot, and standalone waveform self-test.
- `sim::build::generate_model_sources` writes the runtime + libaco sources (plus
  `aco_assert_override.h`) and generated model into a self-contained source tree.
  `build_model_cmake[_with_opts]` compiles or reuses a compatible runtime archive
  and links model-specific sources against it. The generated model passes stack
  headroom through `llg_rt_init_with_args_precision_and_stack` because stack size
  is model-specific and is not part of the runtime archive ABI.

## Requirements

- C11; the model-build line is `cc -std=c11 -O2 -Wall -Wno-unused-function` and must stay
  warning-clean.
- Value operations must not depend on scheduler state, libaco, or waveform output.
  `tests/runtime_values.rs` compiles this module alone and exercises its public
  operations/conversions; generated-model tests cover integration.
  `tests/runtime_value_storage/` independently tests the dynamic storage
  primitive, allocation failures, and waveform transfer/cleanup. It does not
  establish that the remaining legacy value owners have been migrated. Preserve
  `value_sources()` as a header/flat-implementation pair and keep the value facade
  and embedded fragment order synchronized.
- Waveform builds additionally require CMake's `Threads::Threads` and zlib. Thread calls are
  hidden behind a narrow POSIX/Win32 layer; do not use C11 `<threads.h>` as the Windows
  portability boundary. The overall generated simulator still has independent
  native-Windows/libaco limitations.
- The runtime is **timescale-agnostic**: it runs in integer design-precision ticks; codegen
  scales `#N` delays and `$time` reads per the calling module's `timescale` before calling
  `llg_wait_time`/`llg_time_scaled`. Typed `%t` arguments retain their owning module unit for
  the runtime's design-wide `$timeformat` conversion. Integer time queries round by
  quotient/remainder (exact halves upward); `$realtime` remains a separate fractional operation.
  The runtime owns the design-wide `$timeformat` state and converts typed `%t` arguments from
  their owning module units without changing scheduler time.
- Scheduler regions follow the IEEE 1800 §4 fixed-point algorithm: `#0` resumes in Inactive or
  Re-Inactive, NBAs and Re-NBAs retain issue sequence, and Reactive callbacks may enqueue Active
  work for another design iteration. Preponed, Observed and Postponed callback views are
  immutable; illegal writes and read-only callback scheduling report a controlled runtime
  failure. `LLG_ZERO_LOOP_LIMIT` bounds region passes and `LLG_PROCESS_STEP_LIMIT` bounds
  generated loop back-edges (the former also supplies the latter when explicitly set); both
  require positive decimal `uint64_t` values and fail before simulation on invalid/overflow
  input. Verified by the region callback probe, `tests/region_conformance.rs`, and the runtime
  boundary probes.
- Coroutines must never return without `llg_proc_done`/`aco_exit` (the runtime aborts on that —
  codegen bug).

## Interactions

- Above: `src/sim/codegen/` selects runtime operations in IR and `src/sim/emit_c/` emits calls
  into the runtime API; `src/bin/llg.rs` (builds the model and cached runtime via `sim::build`),
  `tests/sim_counter.rs` (`sim_rt_selftest`).
- Below: `vendor/libaco` (coroutine library, embedded in the cached runtime archive, never
  linked into Rust).

## Retained references and sequence endpoints

- Packed queue reference descriptors created by generated calls belong to a LIFO
  `llg_ref_scope_t`. Keep the descriptor and its shared cell alive until call copy-out
  completes; process cancellation unwinds all surviving scopes. Queue structural operations
  snapshot/disconnect removed identities before changing storage. Queue destruction must not
  free a cell still pinned by a call. The queue's reference list is borrowed, not an extra
  owning reference.
- Pending sequence tokens own one transition, a local-value snapshot, and a retained first-match
  scope chain. An entered scope belongs to one invocation; completing it must not discard an
  outer sibling or suffix. Keep all earliest tied endpoints, not whichever work-list node
  happens to be visited first.
- Empty-word alternatives use `admits_empty` and normalized concatenations, not a normal epsilon
  at the current tick. Negative repetition guards are evaluated using normal four-state
  expression truth.
- Consequents inherit endpoint locals by owned declaration ID, not slot number. Initialize only
  consequent-private cells. Snapshot ownership must be released by the common attempt-discard
  path, including abort and shutdown.
- Cross-clock zero delay chooses the nearest destination at-or-after source physical time; one
  delay requires strictly later physical time. Preserve current-slot edge history until all
  relevant endpoints can consume it. A callback ordering index is only a replay guard, never a
  cross-clock delay.
