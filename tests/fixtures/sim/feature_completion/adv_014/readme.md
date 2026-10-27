# ADV-014: global clocking future sampled value functions

These public HDL fixtures cover `$future_gclk`, `$rising_gclk`,
`$falling_gclk`, `$steady_gclk` and `$changing_gclk` (IEEE 1800-2009 16.9.4)
through [`sim_feature_completion::adv_014`](../../../../sim_feature_completion/adv_014.rs).
Positive cases run in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends. Action lines of different assertions in one time
step have no defined order, so each `.out` file is compared as a set of lines.
Every fixture declares `` `timescale 1ns / 1ns ``. Every expected line was
derived by hand from the stimulus and the clauses quoted below, not copied
from a simulator transcript. Line numbers refer to the `pdftotext -layout`
extraction `SystemVerilog-1800-2009.txt`. The portable decision cases
`A14-D1` to `A14-D4` live in `tests/fixtures/sim/lrm_decisions/`.

Run the suite with:

```sh
scripts/run-tests.sh -E 'binary(sim_feature_completion) & test(adv_014::)'
```

## Clauses the expectations rely on

Next global clock tick (16.9.4, L22876-22877, L22916):

> These functions include the capability to access the sampled value at the global clock tick that immediately
> precedes or follows the time step at which the function is called.

> $future_gclk(v) is the sampled value of v at the next global clocking tick.

The status functions (16.9.4, L22919-22927, defined in Annex F.3.4.4,
L70701-70704):

> —      $rising_gclk(e)              b !== 1 && $future_gclk(b) === 1, where b is the LSB of e.
> —      $falling_gclk(e)               b !== 0 && $future_gclk(b) === 0, where b is the LSB of e.
> —      $steady_gclk(e)              e === $future_gclk(e).
> —      $changing_gclk(e)               e !== $future_gclk(e).

Attempt interval, controls and actions (16.9.4, L22949-22964):

> interval of simulation time steps for an evaluation attempt of an assertion containing global clocking future
> sampled value functions is defined as though the future sampled values were known in advance. The end of
> the evaluation attempt is defined to be the last tick of the assertion clock and is not delayed any additional
> time steps up to the next global clocking tick.
>
> The behavior of disable iff and other asynchronous assertion related controls such as $assertkill (see
> 20.11 and 20.12) is with respect to the interval of the evaluation attempt defined above. If, for example,
> $assertkill is executed in a time step strictly after the last tick of the assertion clock for the evaluation
> attempt, then it shall not affect that attempt, even if $assertkill is executed no later than the next global
> clocking tick.
>
> Execution of the action block of an assertion containing global clocking future sampled value functions shall
> be delayed until the global clocking tick that follows the last tick of the assertion clock for the attempt. If the
> evaluation attempt fails and $error is called by default (see 16.15.1), then $error shall be called at the
> global clocking tick that follows the last tick of the assertion clock.

End of a finite trace (Annex F.6.3, L71603-71604):

> $future_gclk(e)[wj] = e[wi+1]. If w is a finite word, $future_gclk(e)[w|w|-1] is undefined.

Illegal contexts (16.9.4, L22874-22875, L22928-22933):

> expression as sampled by the global clock. They may be used only if global clocking is defined (see 14.14).

> The global clocking sampled value functions may be invoked only in property_expr or in sequence_expr;
> this implies that they shall not be used in assertion action blocks.

> the global clocking future sampled value functions: they shall not be nested and they shall not be used in
> assertions containing sequence match items (see 16.10, 16.11).

Operand types of assertion expressions (16.6.1, L21576-21577):

> The following types are not allowed:
> — Noninteger types (shortreal, real, and realtime)

`$assertoff` (20.11, L35048-35049) and `$assertpassoff` (20.12,
L35116-35120):

> $assertoff shall stop the checking of all specified assertions until a subsequent $asserton. An
> assertion that is already executing, including execution of the pass or fail statement, is not affected.

> $assertpassoff shall stop execution of the pass action for vacuous and nonvacuous success of all
> the specified assertions. [...] An assertion that is already executing, including execution of the pass or fail action, is not affected.

## Fixtures

Sampled values below are the Preponed values at the named time.

| Fixture | Stimulus and expected lines |
| --- | --- |
| `values` | Global clock ticks at 5, 15, ..., 55; the run ends at 57. `s` sampled at 5, 15, 25, 35, 45, 55 is 1, 0, 0, 1, 0, 1, so the attempt at tick t of `fut`/`ris`/`fal`/`std`/`chg` compares s(t) with s(t + 10) and reports at t + 10 (L22916, F.3.4.4); `glb` on `$global_clock` matches `chg`. `neg` ticks at 10, 20, ..., 50 and reads s at the next global tick, 5 later (L22876-22877). `vec` ticks at 2, 6, ..., 54 and compares `v` there with `v` at the next global tick by `===` (X bits equal): 2 reports at 5, 6-14 at 15, 18-22 at 25, 26-34 at 35, 38-42 at 45, 46-54 at 55; it fails only where `v` changes before the next global tick (6, 18, 38). `xfl` is `$falling_gclk(v[3])` with v[3] = 0, 0, x, x, 0, 0: it passes only for x to 0 at 35 (`b !== 0 && fut === 0`). `val` passes while the next value is 3 or `4'bx011` (5, 15, 25). Ticks at 55 never report. |
| `intervals` | The gated global clock ticks at 5, ..., 55, 85, 95, ..., 165 (`en` low 62-82). `imp` (`a \|=> $steady_gclk(b)` on slow ticks 20, 60, 100, 140) passes at the global tick after the consequent tick: 85, 105, 145. `vac` succeeds vacuously at every slow tick and reports at 25, 85, 105, 145; `cov` hits once, at 25 (b rises from 0 at 20 to 1 at 25). `rng` fails at 55 for the attempt at 25 (no falling `b` at 35 or 45). `mcl` checks `$rising_gclk(c)` at the first clk tick after each slow tick (25, 65, 105, 145); only 145 to 155 rises (c is set at 150); reports at 35, 85, 115, 155. |
| `composition` | `ifs` fails only for the tick at 5 (b at 15 is 0, and `a` never changes); `syn` is accepted when c (set at 150) holds at 155, for the attempts at 145 and 155 (both at 165), and every other attempt fails silently through its null else; `ini` is the single attempt of an initial procedure, begun at 5 and failing at 25 (b steady from 25 to 35); `prc` is a procedural assertion clocked by its procedure's `posedge slow` (20, 60, 100, 140, reported at 25, 65, 105, 145); `loc`'s local input formal takes b at the attempt's start: 0 at 20 (fail at 25), then 1. |
| `end` | The last global tick is 45; the run ends at 47 (A14-D3). `e1` reports 5-35 at 15-45 and never the tick at 45. `e2` (`s \|-> s_eventually $rising_gclk(r)`, r constant) leaves four evaluated attempts pending, which fail at 47 (S38-D1, A13-D7). `e3`'s strong consequent completes for the attempts at 5 and 15 (reported at 35, 45); those at 25 and 35 need ticks that are never evaluated and end without a result. `e4` ticks at 10-40. |
| `controls` | Slow ticks 20, 60, 100, 140; each attempt ends at the next one and reports 5 later (A14-D2). `kin` is killed at 62: the attempt ending at 60 still reports at 65, the one begun at 60 is discarded, `$asserton` at 80 restores the 100 attempt (145). `kaf` is killed at 102, after the attempt ending at 100 (reported at 105). `dis` discards the attempt begun at 60 for `rst` at 62; `dat`'s `rst2` rises in the time step of the tick at 100, which discards the attempt ending there and disables the one begun there. `pof`'s `$assertpassoff` at 62 comes after the tick at 60 and does not suppress the pass at 65; `pon`'s comes at 58, before it, and its `$assertpasson` at 62 does not restore it (A14-D4). `off`'s `$assertoff` at 102 does not stop the attempt begun at 100 (145). |
| `budget` | Ten assertion ticks per global period stay pending at once and reuse their buffers; while the global clock is gated off (2000-4000) they accumulate and count against `LLG_SEQUENCE_THREAD_LIMIT`, which stops the run with 50 at time 2091 (51 pending ticks since 1990). Without a limit all 2045 ticks before the last global tick (4090) report. |
| `neg_nested`, `neg_match_item`, `neg_procedural`, `neg_action`, `neg_no_global` | The 16.9.4 restrictions quoted above: frontend diagnostics at the call. |
| `neg_real` | A real argument (16.6.1): llg diagnostic. |
| `neg_expect`, `neg_async_abort`, `neg_endpoint` | Located llg limits: an `expect` would block its caller until the next global tick, an asynchronous `accept_on`/`reject_on` acts between assertion clock ticks, and a sequence end point reports a match in the time step it occurs. |
