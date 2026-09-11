# sim::rt — embedded C11 simulation runtime

## Purpose

The C11 runtime that executes generated models.  It is compiled together with
the generated `model.c` into a standalone executable and is deliberately
**never linked into the Rust binaries**:

- `llg_value.h` / `llg_value.c` — scheduler-independent data types, operations,
  formatting, and numeric conversions, compiled as a standalone C11 translation
  unit with only standard C/math dependencies:
  - `sv4_t` — up to the generated model's `LLG_MAX_WIDTH` bits (below the
    backend's exclusive `1 << 20` limit) stored as three parallel
    64-bit limb arrays (`bits`/`x`/`z`), with X and Z kept distinct
    (`x & z == 0`).  Z behaves as X in every unknown-propagating op (LRM
    11.4.5) but is carried through identity/copy ops and distinguished by
    `$display`, casez/casex wildcards and `===`/`!==`.
  - Value ops — arithmetic/logic/reduction/compare/wildcard-equality/casez/casex, mux, concat,
    repeat, part/bit/indexed-part selects, resize/fill/clog2, format and
    decimal conversion; partially out-of-range part-select reads retain valid
    bits and fill missing positions with X. Semantics mirror `core::elab::Value` (kept in sync).
    `sv4_resolve` combines equal-strength driver values independently of the
    scheduler; `sv4_resolve_strengths` additionally applies per-driver 0/1
    endpoints for ordinary wires. An X contribution represents both endpoint
    ranges, so a known driver resolves the bit only when it strictly dominates
    every possible opposite value. Wire conflicts yield X, wired-AND 0
    dominates X, wired-OR 1 dominates X, and Z is neutral in every mode. No active drivers yield Z.
    Tri0/tri1 modes instead fill undriven bits with their pull defaults;
    supply0/supply1 modes dominate ordinary implicit-strength contributions.
    Bit-vector queries count known one bits across all limbs, ignore X/Z for
    `$countones`/`$onehot`/`$onehot0`, and detect either state for `$isunknown`.
  - Real-number hooks — packed-to-`double` conversion across all `sv4_t`
    limbs (X/Z bit positions contribute zero), rounded `double`-to-packed
    conversion (targets up to the model width), scalar truth conversion, wide
    division/modulo/power, and `%f`/`%e`/`%g` formatting support the procedural
    scalar real/shortreal contract. `shortreal` precision is enforced by
    codegen at assignments and initialization; unsupported double-aware
    scheduling contexts are rejected before generated C is compiled.
    `$rtoi` truncates rather than using assignment rounding; real/shortreal
    bitcasts use `memcpy` and require 64-bit `double`/32-bit `float` storage.
    Delay conversion accepts explicit unit/precision tick scales: packed X/Z
    maps to zero, negative packed values convert to unsigned 64-bit time, and
    real values round once at local precision. Scaling overflow and nonfinite
    or negative real delays produce fatal diagnostics before scheduling.
    See the [lowering guide](../codegen/AGENTS.md) for conversion bounds.
- `llg_rt.h` / `llg_rt.c` — event scheduler and simulation-facing services.
  The header includes `llg_value.h` as a source-compatible facade; the C
  implementation links value operations rather than including their source.
  Libaco coroutines execute an IEEE 1800 §4 region loop (active → inactive
  `#0` → NBA commit → re-run woken processes → next timed wakeup).
  Services include edge/event/level waits, fork/join, blocking/nonblocking
  packed and double assignments, net resolution, force/release, and
  `$display`/`$monitor`/`$strobe`/`$finish`/`$time`.
  Completed fork parents remain alive until detached descendants finish;
  process-table slots are reused, and allocations are released on scheduler
  exit or reinitialization.
- `llg_string.h` / `llg_string.c` — scheduler-independent, owned byte strings.
  Reads clone storage; expression operations consume their arguments; mutations
  replace or update the target allocation. Strings exclude NUL bytes and use
  ASCII case conversion. `atoreal` parses a decimal prefix without admitting C
  hexadecimal/NaN/infinity spellings; `realtoa` emits enough decimal digits for
  finite-double round trips. Both preserve the existing clone/consume contract.
- `llg_container.h` / `llg_container.c` — scheduler-independent storage for
  dynamic arrays, queues, and associative arrays whose packed elements are
  full-width `sv4_t` values. Dynamic resize preserves the common prefix and
  default-fills growth; bounded queues apply the LRM discard rules;
  associative arrays keep ordered integral or byte-string keys. All three
  container kinds provide packed-element reduction folds. Every object
  requires matching init/destroy calls. Allocation overflow and exhaustion are
  fatal diagnostics, while invalid indices use default-read/no-op-write method
  semantics. Integral associative keys containing X/Z are rejected before
  declared-key casting.
- `llg_rt_selftest.c` — C self-tests: sv4 value vectors (mirrored from the
  `core::elab` unit tests) plus scheduler checks (delay ordering, NBA
  visibility, ping-pong, directly observed nested `join_none` lifetimes, empty
  fork-group finalization, and cumulative process slot reuse). Compiled and run
  by the `sim_rt_selftest` case in `tests/sim_counter.rs`.
- `llg_wave.h` / `llg_wave.c` — optional VCD/FST waveform runtime. The one OS
  simulation thread is the sole producer of a bounded SPSC ring and a
  dedicated POSIX/Win32 writer thread is the sole consumer and file owner.
  Events own their values, publication uses release/acquire ordering, full and
  empty waits use condition variables, `$dumpflush` is an acknowledged FIFO
  barrier, and close is an in-band event followed by join. Never add a second
  producer without replacing this contract or assigning it a separate SPSC
  ring. Registration names use ASCII unit separator (`0x1f`) between
  hierarchy components; it cannot occur in a source identifier. The writer
  reversibly encodes punctuation per component, preventing escaped dots from
  becoming scopes and preventing serialized-name collisions.
- `gtkwave/` — pinned GTKWave libfst writer/reader plus its FastLZ/LZ4 support
  and license/provenance files. It is emitted and compiled only for models
  containing waveform controls. FST compression uses zlib and libfst's own
  internal parallel mode stays disabled because `llg_wave.c` owns threading.
- `llg_wave_selftest.c` — forces ring wrap/backpressure, checks the VCD flush
  barrier, writes both formats, and reopens the FST with the official reader.

## Scheduling and value details

`sv4_t` has model-sized `uint64_t bits[]`, `x[]`, `z[]`, a `uint32_t width`, and
`int8_t is_signed`. Keep these operations aligned with `core::elab::Value`;
[../../../tests/AGENTS.md](../../../tests/AGENTS.md) describes property/vector checks.

One coroutine runs each always/initial (including generated scopes),
continuous assignment and port link; fork branches use `llg_fork`. Active
coroutines are FIFO. Immediate NBA lists and the global timed NBA queue commit
in issue order within the NBA region, re-iterating to quiescence before advancing
time. Future NBAs own captured values and persistent destination pointers,
remain scheduled after process completion, and can advance time without a timed
process waiter. Masked NBA writes merge only selected bits into current storage.
Inertial drivers own one pending active-region update per site and remain live
after their evaluation process ends. A changed pending value cancels its event;
an unchanged value retains its deadline; returning to the current contribution
cancels without replacement. The sorted event queue advances time independently
of process waiters. Static generated handles are reset by cleanup before their
runtime-owned driver storage is freed; reinitialization releases pending events.
Zero-delay driver updates drain in the active region. Strobe and change-driven
monitor checks run only after active/inactive/NBA work has reached quiescence.
The scheduler stops on `$finish` or deadlock.
Per-waiter snapshots detect posedge 0→1, 0→X/Z, X/Z→1 (negedge mirrored), using
only the LSB for packed vector edges. Expression waits own copied dependency
lists and value snapshots; `iff` callbacks run at the trigger, including named
events. Wakeup, disable and teardown free these allocations and unregister all
named events. A zero-dependency signal wait remains suspended without polling.
`llg_wait_any` uses snapshots; event or-lists require atomic
`llg_wait_any_events`, never sequential waits. See the lowering guide for
force/release and inout resolution approximations.
Net groups select wire/wired-AND/wired-OR/pull/supply resolution through the standalone
value API; `llg_net_write` publishes only resolved-value changes to waiters.
Generated driver cells start at Z. Lowering explicitly writes X into an
existing delayed continuous-driver slot before processes start, distinguishing
that pending driver from a genuinely driverless Z net. Selected continuous
drivers publish a fresh Z-based contribution on every evaluation, so only the
selected range contributes; lowering rejects dynamic net selectors.

## Embedding

- All sources are embedded as strings via `include_str!` in `mod.rs`:
  - `value_sources()` → `(llg_value.h, llg_value.c)`;
  - `runtime_sources()` → `(llg_rt.h, llg_rt.c)`, requiring the value pair;
  - `string_sources()` → `(llg_string.h, llg_string.c)`, requiring the value pair;
  - `container_sources()` → `(llg_container.h, llg_container.c)`, requiring
    the value pair but not the scheduler;
  - `libaco_sources()` → `(aco.h, aco.c, acosw.S)` from `vendor/libaco`;
  - `selftest_source()` → `llg_rt_selftest.c`.
  - `waveform_sources()` / `waveform_selftest_source()` → the optional
    waveform runtime, libfst snapshot, and standalone waveform self-test.
- `sim::build::generate_model_sources` / `build_model_cmake[_with_opts]`
  write the runtime + libaco sources (plus `aco_assert_override.h`) and the
  extra sources (generated model or selftest) into a build directory and
  build them with CMake, so libaco is compiled together with the model at
  model-build time.

## Requirements

- C11; the model-build line is `cc -std=c11 -O2 -Wall -Wno-unused-function`
  and must stay warning-clean.
- Value operations must not depend on scheduler state, libaco, or waveform
  output. `tests/runtime_values.rs` compiles this module alone and exercises
  its public operations/conversions; generated-model tests cover integration.
- Waveform builds additionally require CMake's `Threads::Threads` and zlib.
  Thread calls are hidden behind a narrow POSIX/Win32 layer; do not use C11
  `<threads.h>` as the Windows portability boundary. The overall generated
  simulator still has independent native-Windows/libaco limitations.
- The runtime is **timescale-agnostic**: it runs in integer design-precision
  ticks; the codegen scales `#N` delays and `$time`/`%t` reads per the
  calling module's `timescale` before calling `llg_wait_time`/`llg_time`.
- Scheduler region model follows the implemented IEEE 1800 §4 subset
  (active/inactive/NBA; observed/reactive regions are absent): `#0` resumes in
  the inactive region between active and NBA, and a per-time-step iteration
  counter trips `LLG_ZERO_LOOP_LIMIT` zero-delay loops. Verified by
  `tests/region_conformance.rs`.
- Coroutines must never return without `llg_proc_done`/`aco_exit` (the
  runtime aborts on that — codegen bug).

## Interactions

- Above: `src/sim/codegen/` selects runtime operations in IR and
  `src/sim/emit_c/` emits calls into the runtime API;
  `src/bin/llg.rs` (builds model + runtime + libaco via
  `sim::build`), `tests/sim_counter.rs` (`sim_rt_selftest`).
- Below: `vendor/libaco` (coroutine library, embedded and compiled with the
  model, never linked into Rust).
