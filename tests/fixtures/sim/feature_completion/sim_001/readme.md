# SIM-001 region iteration and procedural time contracts

IEEE 1800-2009 §§3.14, 4.4–4.7, 9.4.1, 16.5, 20.4 and 24.3, and IEEE
1364-2001 §9.7.1 supply the oracles; every expectation is computed by hand
from those clauses. Every source runs through the public CLI in both optimizer
modes on the legacy and compact (portable and GMP)
value backends.

Outputs whose interprocess order the standard leaves open are not compared as
one string. `region_reentry` is checked as a line multiset plus the ordering
chains the §4.5 reference algorithm fixes; `race_outcomes` accepts every
permitted outcome at each racy line.

- `region_reentry`: Active -> Inactive (`#0`, twice) -> NBA -> Active, and
  program Reactive -> Re-Inactive (`#0`, twice) -> Re-NBA -> Active re-entry,
  with design work enabled by the reactive set waiting until that set drains.
- `region_pingpong`: three program/module handshakes in one time slot, each
  crossing Reactive -> Re-NBA -> Active -> NBA -> Reactive; a `$strobe` sees
  the final values.
- `observed_reentry`: an assertion clocked by a signal only program code
  changes is still evaluated in Observed of the same slot (§24.3.1); the
  property reads Preponed samples while the action block reads current values.
- `race_outcomes`: write/write and read/write races in Active accept either
  value; initializer, time-zero continuous assignment, NBA, `$strobe` and
  Reactive visibility are fixed.
- `time_units`: `#1` under every unit 1/10/100 of s, ms, us, ns, ps and fs
  lands exactly one local unit after zero; `%t` reports femtoseconds.
- `delay_rounding`: constant, time-literal and runtime real delays round half
  away from zero at a 100ps local precision inside a 1ps design; sub-precision
  delays stay in the same slot, X/Z delays are zero, a runtime delay that
  changes between loop iterations is re-evaluated each time and a function
  delay is evaluated once.
- `negative_real_delay`, `negative_real_constant`: finite negative real delays
  are two's-complement unsigned 64-bit time after local rounding.
- `time_limit`, `time_limit_nba`: a delay reaching exactly 2^64-1 schedules;
  one past it fails before queue insertion (owner resource policy).
- `neg_negative_real_overflow`: a negative real whose unsigned time cannot be
  scaled to the design tick reports the tick-range limit, not illegality.
- `neg_nonfinite_delay`: a nonfinite real delay is rejected at run time.
- `neg_function_delay`, `neg_program_always`: language-illegal neighbours
  (a delay in a function, an always procedure in a program).
