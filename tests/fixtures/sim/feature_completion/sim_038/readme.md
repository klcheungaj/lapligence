# SIM-038 ordinary properties, actions and expect

IEEE 1800-2009 §§16.12-16.18 and Annex F supply the oracles. Every positive
source runs through the public CLI in both optimizer modes on the legacy and
compact (portable and GMP) value backends. Output lines are compared as sorted
multisets: the order of actions of different assertions in one time step is
not fixed by the LRM. Times print in picoseconds of the default precision
(tick 1 at 5 ns prints `5000`).

Clause quotes cite `SystemVerilog-1800-2009.txt` line numbers.

Rules used throughout:

- Strength defaults. 16.13.1 (L24486-24489): "If the strong or weak operator
  is omitted, then the evaluation of the sequence_expr depends on the
  assertion statement in which it is used. If the assertion statement is
  assert property or assume property, then the sequence_expr is evaluated as
  weak(sequence_expr). Otherwise, the sequence_expr is evaluated as
  strong(sequence_expr)."
- Vacuity. 16.15.8 (L27316-27466) lists the rule per operator, for example
  e) (L27326-27328): "An evaluation attempt of a property of the form
  property_expr1 or property_expr2 is nonvacuous if, and only if, either the
  underlying evaluation attempt of property_expr1 is nonvacuous or the
  underlying evaluation attempt of property_expr2 is nonvacuous." A pass
  statement runs for vacuous and nonvacuous successes of an assert; a
  `cover property` counts (and runs its statement for) nonvacuous successes
  only.
- Aborts. 16.13.14 (L25255-25258): "If during the evaluation, the abort
  condition becomes true, then the overall evaluation of the property results
  in true." (false for `reject_on`); L25265-25267: "The operators accept_on
  and reject_on are evaluated at the granularity of the simulation time step
  like disable iff but their abort condition is evaluated using sampled value
  as a regular Boolean expression in assertions." 16.15.8 ab) (L27439-27441):
  accept_on is nonvacuous only if "expression_or_dist does not hold in any
  time step of that evaluation attempt" (decisions S38-D2, S38-D9).
- Disable. 16.15.1 (L26432-26434): "When the property for the assert
  statement is evaluated to be disabled, no action_block statement is
  executed." 16.16 (L27555-27556): "If an assertion has a disable iff clause,
  then the disable condition specified in this clause shall be used and any
  default disable iff declaration ignored for this assertion." (S38-D5).
- No else. 16.15.1 (L26455-26456): "If no statement is specified for else,
  then $error is used as the statement when the assertion fails." llg reports
  it on stderr as `llg: assertion assert failed: <location> (<label>)`.
- End of simulation (S38-D1): a pending attempt with an unmet strong
  obligation fails when the simulation ends; one with only weak obligations
  ends without a result.

## A01 `exhaustive`

45 properties over `a` and `b` (labels `p00`-`p44`) cover sequence properties
with explicit and implicit strength, `not`, `and`, `or`, `implies`, `iff`,
`|->`/`|=>` with property consequents, `#-#`/`#=#`, `if`/`if-else`, `case`,
`nexttime`/`s_nexttime` with counts, bounded and unbounded `always`,
`s_always`, `eventually`, `s_eventually`, the four `until` forms, the four
abort forms and nestings of them. For each of the 1024 two-signal traces of
length 5 the testbench raises `go` on tick 0, drives the trace on ticks 1-5
and disables pending attempts with `disable iff (kill)` on tick 6. Each
property runs as `assert property (go |=> P)` printing `A`/`F` lines for pass
and fail, and as `cover property (go |=> P)` printing `C` lines for
nonvacuous successes; attempts started with `go` low succeed vacuously on
their own tick (an `A` line without a `C` line).

The expected lines come from a test-side interpreter in
`tests/sim_feature_completion/sim_038.rs` that shares no code with the
runtime. It evaluates Annex F neutral satisfaction (F.5.3.1) on words over
`2^{a,b}` plus the letters T and ⊥, expanding derived operators by F.3.4.3.
An attempt is reported on the first tick whose trace prefix decides it: every
continuation `u v c^ω` (`|v| <= 2`) gives the same result. A pass is
nonvacuous when the 16.15.8 rules hold on the finite word of its ticks. An
attempt no prefix decides stays pending and prints nothing. The test checks
that the fixture spells each interpreter property exactly.

## A01 `outcomes`

Ticks 1-8 are the posedges at 5..75; `$finish` ends the run at 80.

| tick | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| s1, s3 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| b, h | 0 | 1 | 1 | 0 | 0 | 0 | 0 | 0 |
| s2 | 1 | 0 | 0 | 1 | 0 | 0 | 0 | 0 |
| e | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |
| s4 | 0 | 0 | 0 | 0 | 1 | 0 | 0 | 0 |
| h2 | 0 | 0 | 0 | 0 | 0 | 1 | 1 | 0 |
| s5 | 0 | 0 | 0 | 0 | 0 | 1 | 0 | 0 |

`c` and `k` stay 0, `k2` stays 1. Asynchronous pulses: `rst` 1 during
[22, 23), `g` 1 during [27, 28), `g2` rises and falls at 62 in one time step.

- `n1` `s1 |=> (b until c)`, no action block: `b` holds on ticks 2-3 and
  fails on tick 4 with `c` still 0, so the attempt fails at 35 and `$error`
  runs: one stderr failure line for `n1`.
- `d1`/`d1c` `disable iff (rst) s2 |=> s_eventually [0:4] e`: the tick-1
  attempt (window ticks 2-6) is disabled by the `rst` pulse at 22 before `e`
  on tick 6; the tick-4 attempt (window 5-9) sees `e` on tick 6. Output: one
  `55000 d1 cover`, no `d1 fail` (without the pulse there would be two
  covers).
- `ab1` `s3 |=> accept_on (g) always [0:3] h`: the tick-1 attempt would fail
  on tick 4 (`h` 0), but the sampled value of `g` is 1 in time step 28, so
  it succeeds vacuously at 28. The other attempts have `s3` 0 and pass
  vacuously on their tick: `ab1 pass` at 15, 25, 28, 35, 45, 55, 65, 75;
  `ab1c` prints nothing.
- `ab2` `sync_accept_on (g)`: the condition is sampled only at clock ticks
  and is 0 on every tick, so the attempt fails at 35.
- `rj1` `reject_on (g)`: fails at 28.
- `ab3` `s4 |=> accept_on (g2) always [0:2] h2`: the glitch at 62 is never a
  sampled 1, so the attempt (ticks 6-8) fails at 75 when `h2` is 0.
- `es` `s5 |=> s_eventually k`: pending at the end with a strong obligation,
  fails at 80. `sn` `s5 |=> s_nexttime [4] k2` needs tick 11: fails at 80.
  `ens` `s5 |=> (k2 s_until k)` fails at 80 through `$error` (second stderr
  failure line).
- `ew` `s5 |=> always k2` and `wn` `s5 |=> nexttime [4] k`: weak and pending
  at the end, no result. `ew` prints only the vacuous passes of the other
  attempts (5, 15, 25, 35, 45, 65, 75), never a pass at 80.
- `ce` `cover property (s5 |=> s_eventually k)`: never covered.

## A02 `actions`

Ticks as above. `s` is 1 on ticks 1-2, `b` on ticks 2-3.

- `pa` `s |=> (always [0:1] b)`: the tick-1 attempt passes at 25 (`b` on
  ticks 2-3); the tick-2 attempt fails at 35 (`b` 0 on tick 4); attempts of
  ticks 3-8 pass vacuously on their tick. Every pass action prints `begin`,
  waits 12 and prints `end`; every fail action waits 7. Each execution is its
  own process (S38-D7), so the two pass actions of 25 both end at 37 while the
  fail action of 35 ends at 42; the action begun at 75 is still waiting at
  `$finish`.
- `wait_for` (automatic task) runs `expect (s ##1 b)` and assigns its
  automatic output from the action block, which runs in the caller (S38-D4).
  Called at 2, the evaluation starts on tick 1 and matches on tick 2 (15); the
  pass statement waits 3 and sets `ok`, then `resumes++`: `18 e1 ok=1
  resumes=1`. Called again at 18, it starts on tick 3 where `s` is 0 and fails
  at 25: `25 e2 ok=0 resumes=2`. 16.18 (L27951-27952): "The statement
  following the expect is scheduled to execute after processing the Observed
  region in which the property completes its evaluation."
- The second process runs `expect (nexttime (always [0:1] !b))` at 30: it
  starts on tick 4 and needs `!b` on ticks 5-6, so it passes at 55 and
  resumes once.

## A01 `disable`

Every attempt starts with `s` on tick 1 (5) and fails on tick 5 (45) unless a
pulse disables it: `rst_n` low during [22, 23), `kill_m` high during [27, 28),
`d` high during [32, 33).

- `x0` `disable iff (1'b0)` overrides the module default: fails at 45.
- `x1` `disable iff (!rst_n)` and `x2` `disable iff (c || d)` (general
  expressions, 16.12) are disabled.
- `y1` has no `disable iff` and inherits `default disable iff (kill_m)`:
  disabled.
- `g.y2` sits in a generate block whose own default `kill_g` (never 1)
  overrides the module's: fails at 45.
- `u_child.c1` is in a child instance, which the module default does not
  reach (16.16 L27500-27501, "The scope does not extend into any instances of
  modules"): fails at 45.
- The `expect (s ##1 !t ##1 !t ##1 !t)` started at 2 does not inherit the
  default (S38-D5) and passes at 35 despite the `kill_m` pulse.

## `initial_assert`

`a` is 1 on ticks 1-3, `b` on tick 2. `i1` (`a ##1 b`) and `i2` (`a |=> b`)
are whole `initial` procedures and start one attempt on tick 1 (S38-D6),
passing at 15. The static `s1` (`a ##1 b`) passes at 15 (tick-1 attempt) and
fails at 25 (tick 2), 35 (tick 3) and 35 (tick 4, `a` 0).

## A03 negatives

- `neg_s_always_unbounded`, `neg_weak_eventually_unbounded`: `s_always` and
  weak `eventually` need a bounded range (16.13.11, 16.13.13); Slang reports
  "unbounded literal '$' not allowed here".
- `neg_nested_disable`: `disable iff` inside a property operator is not
  grammar (16.12); Slang reports "expected expression".
- `neg_empty_match_property`: a sequence property must not admit an empty
  match (16.13.1 L24484); Slang reports it.
- `neg_multiclock_property`, `neg_conflicting_clock`: a clock inside the
  property (written or from a named property) that differs from the leading
  clock makes a multiclock property (16.14), which llg rejects with
  "multiclock properties are not supported".
- `neg_recursive_property`: recursive property instances (16.13.17) are
  rejected with "recursive property instances are not supported".
