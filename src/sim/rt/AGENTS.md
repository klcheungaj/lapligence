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
  - Value ops — arithmetic/logic/reduction/compare/casez/casex, mux, concat,
    repeat, part/bit/indexed-part selects, resize/fill/clog2, format and
    decimal conversion; semantics mirror `core::elab::Value` (kept in sync).
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
- `llg_rt_selftest.c` — C self-tests: sv4 value vectors (mirrored from the
  `core::elab` unit tests) plus scheduler checks (delay ordering, NBA
  visibility, ping-pong, directly observed nested `join_none` lifetimes, empty
  fork-group finalization, and cumulative process slot reuse).  Compiled and run by `tests/sim_counter.rs`
  `sim_rt_selftest`.

## Embedding

- All sources are embedded as strings via `include_str!` in `mod.rs`:
  - `runtime_sources()` → `(llg_rt.h, llg_rt.c)`;
  - `libaco_sources()` → `(aco.h, aco.c, acosw.S)` from `vendor/libaco`;
  - `selftest_source()` → `llg_rt_selftest.c`.
- `sim::build::generate_model_sources` / `build_model_cmake[_with_opts]`
  write the runtime + libaco sources (plus `aco_assert_override.h`) and the
  extra sources (generated model or selftest) into a build directory and
  build them with CMake, so libaco is compiled together with the model at
  model-build time.

## Requirements

- C11; the model-build line is `cc -std=c11 -O2 -Wall -Wno-unused-function`
  and must stay warning-clean.
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
