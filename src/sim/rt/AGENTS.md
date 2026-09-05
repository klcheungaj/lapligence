# sim::rt — embedded C11 simulation runtime

## Purpose

The C11 runtime that executes generated models.  It is compiled together with
the generated `model.c` into a standalone executable and is deliberately
**never linked into the Rust binaries**:

- `llg_rt.h` / `llg_rt.c` — the `sv4_t` 4-state value model and the event
  scheduler:
  - `sv4_t` — up to `LLG_MAX_WIDTH` (1024) bits stored as three parallel
    64-bit limb arrays (`bits`/`x`/`z`), with X and Z kept distinct
    (`x & z == 0`).  Z behaves as X in every unknown-propagating op (LRM
    11.4.5) but is carried through identity/copy ops and distinguished by
    `$display`, casez/casex wildcards and `===`/`!==`.
  - Value ops — arithmetic/logic/reduction/compare/wildcard-equality/casez/casex, mux, concat,
    repeat, part/bit/indexed-part selects, resize/fill/clog2, format and
    decimal conversion; semantics mirror `core::elab::Value` (kept in sync).
    Bit-vector queries count known one bits across all limbs, ignore X/Z for
    `$countones`/`$onehot`/`$onehot0`, and detect either state for `$isunknown`.
  - Scheduler — libaco coroutines per process; an IEEE 1800 §4 region loop
    (active region → inactive region (`#0`, drained in a loop) → NBA commit →
    re-run woken processes → advance time to the next timed wakeup),
    per-waiter edge detection, `llg_wait_time`/`llg_wait_edge`/
    `llg_wait_any`/`llg_wait_any_events`/`llg_wait_level`, fork/join
    (`llg_fork`/`llg_join`/`llg_wait_fork`/`llg_disable_fork`), blocking
    (`llg_ba`) and non-blocking (`llg_nba`) assignments, and
    `$display`/`$monitor`/`$strobe`/`$finish`/`$time` support. Completed
    fork parents remain alive until detached descendants finish, reclaimed
    process-table slots are reused, and runtime allocations are released on
    scheduler exit or reinitialization.
  - Real-number hooks — packed-to-`double` conversion across all `sv4_t`
    limbs (X/Z bit positions contribute zero), rounded `double`-to-packed
    conversion (targets up to 64 bits), blocking/non-blocking double assignment,
    scalar truth conversion, and `%f`/`%e`/`%g` formatting support the
    procedural scalar real/shortreal B6 contract.  `shortreal` precision is
    enforced by codegen at assignments and
    initialization; unsupported double-aware scheduling contexts are rejected
    before generated C is compiled.
    `$rtoi` truncates rather than using assignment rounding; real/shortreal
    bitcasts use `memcpy` and require 64-bit `double`/32-bit `float` storage.
    See the [lowering guide](../codegen/AGENTS.md) for conversion bounds.
- `llg_rt_selftest.c` — C self-tests: sv4 value vectors (mirrored from the
  `core::elab` unit tests) plus scheduler checks (delay ordering, NBA
  visibility, ping-pong, directly observed nested `join_none` lifetimes, empty
  fork-group finalization, and cumulative process slot reuse).  Compiled and run by `tests/sim_counter.rs`
  `sim_rt_selftest`.
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

`sv4_t` has `uint64_t bits[16], x[16], z[16]`, `uint16_t width`, and
`int8_t is_signed`. Keep these operations aligned with `core::elab::Value`;
[../../../tests/AGENTS.md](../../../tests/AGENTS.md) describes property/vector checks.

One coroutine runs each always/initial (including generated scopes),
continuous assignment and port link; fork branches use `llg_fork`. Active
coroutines are FIFO; NBA commits per-process `llg_nba` lists, re-iterating to
quiescence before advancing time, and stops on `$finish` or deadlock.
Per-waiter last-seen values detect posedge 0→1, 0→X, X→1 (negedge mirrored).
`llg_wait_any` uses snapshots; event or-lists require atomic
`llg_wait_any_events`, never sequential waits. See the lowering guide for
force/release and inout resolution approximations.

## Embedding

- All sources are embedded as strings via `include_str!` in `mod.rs`:
  - `runtime_sources()` → `(llg_rt.h, llg_rt.c)`;
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
- Waveform builds additionally require CMake's `Threads::Threads` and zlib.
  Thread calls are hidden behind a narrow POSIX/Win32 layer; do not use C11
  `<threads.h>` as the Windows portability boundary. The overall generated
  simulator still has independent native-Windows/libaco limitations.
- The runtime is **timescale-agnostic**: it runs in integer design-precision
  ticks; the codegen scales `#N` delays and `$time`/`%t` reads per the
  calling module's `timescale` before calling `llg_wait_time`/`llg_time`.
- Scheduler region model follows IEEE 1800 §4 (active/inactive/NBA/reactive;
  observe/reactive are structurally empty in v1): `#0` resumes in the
  inactive region between active and NBA, and a per-time-step iteration
  counter trips `LLG_ZERO_LOOP_LIMIT` zero-delay loops.  Verified by
  `tests/region_conformance.rs`.
- Coroutines must never return without `llg_proc_done`/`aco_exit` (the
  runtime aborts on that — codegen bug).

## Interactions

- Above: `src/sim/codegen.rs` (emits calls into the runtime API),
  `src/bin/llg.rs` (builds model + runtime + libaco via
  `sim::build`), `tests/sim_counter.rs` (`sim_rt_selftest`).
- Below: `vendor/libaco` (coroutine library, embedded and compiled with the
  model, never linked into Rust).
