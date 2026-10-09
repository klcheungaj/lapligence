# SIM-037 single-clock sequence composition

IEEE 1800-2009 §§16.7-16.10 and Annex F supply the oracles. Every positive
source runs through the public CLI in both optimizer modes on the legacy and
compact (portable and GMP) value backends. Output lines are compared as sorted
multisets: the order of actions of different assertions in one time step is
not fixed by the LRM, but the number of lines is (one line per match).

Notation: a sequence that starts on tick `s` and whose match ends on tick `e`
is written `[s, e]`; an empty match of a sequence started on tick `s` ends on
tick `s - 1`. Rules used throughout (§16.9.2.1, Annex F):

- `r1 ##1 r2`: `r2` starts the tick after `r1` ends; `r1 ##0 r2` overlaps one
  tick, so both operands must be nonempty there. `(empty ##n r)` is
  `##(n-1) r` and `(r ##n empty)` is `r ##(n-1) 1`; `(empty ##0 r)` and
  `(r ##0 empty)` never match.
- `r[*0]` is the empty sequence; `r[*k+1]` is `r[*k] ##1 r`.
- A leading `##[m:n] r` is `1[*m:n] ##1 r`. F.3.4.2.2
  (`SystemVerilog-1800-2009.txt` L70581): "( ##[m:n] R ) (1[*m:n] ##1 R )"
  (the PDF's equivalence sign is lost in the text extraction).
  With `m = 0` the `1[*0] ##1 r` term is `r` itself, so `##[0:n] r` keeps
  the empty match of `r` exactly like `##0 r` (§16.7 L21795: "##0 a // means
  a"). `s35` (`##[0:1] (a)[*0:1]` after `go ##1`) therefore also ends on the
  `go` tick 0 for every trace.
- `r1 or r2`: the union of the match sets; one line per distinct end tick
  (an attempt has a set of matches, not a list).
- `r1 and r2`: both start on the same tick; a match ends at the later of one
  end of each operand. An empty operand match ends before the start, so it
  never moves the end.
- `r1 intersect r2`: both start on the same tick and end on the same tick.
- `e throughout r` is `e[*0:$] intersect r`; `r1 within r2` is
  `(1[*0:$] ##1 r1 ##1 1[*0:$]) intersect r2`.
- `first_match(r)`: only the earliest end tick of the attempt, once.
- `cover sequence (go ##1 (r))` prints one line per match of `r` started on
  the tick after `go`. IEEE 1800-2009 16.15.3
  (`SystemVerilog-1800-2009.txt` L26590-26592): "for sequence coverage, all
  matches per evaluation attempt are reported, whereas for property coverage
  the coverage count is incremented at most once per evaluation attempt". A
  `cover property` of `go ##1 (r) |-> 1'b1` would report at most one line
  per attempt.

## A01 `exhaustive`

`exhaustive.sv` holds 36 sequences over `a` and `b` (labels `s00`-`s35`):
concatenation with fixed, ranged, unbounded and leading delays, `##0`
fusion, every repetition form (including `[*0]`, `[*0:$]`, repeated
sequences, repeated empty-admitting bodies, goto and nonconsecutive), `or`,
`and`, `intersect`, `throughout`, `within`, `first_match` and nestings of
them. For each of the 1024 two-signal traces of length 5 the testbench pulses
`go` on tick 0, drives the trace on ticks 1-5 and kills pending attempts with
`disable iff (kill)` on tick 6. Each match prints `<label> <trace> <end>` from a `cover sequence`.

The expected lines are computed by the test from a separate trace
interpreter in `tests/sim_feature_completion/sim_037.rs`, written directly
from the Annex F tight-satisfaction rules above on finite words (it shares no
code with the simulator's automaton). The test also checks that the fixture
spells each interpreter sequence exactly, so the two cannot drift apart.

## A02 `match_sets`

`go` is high on tick 0 only, so every sequence starts on tick 1. `go3` is high
on ticks 1-3 (`m12` starts one attempt per tick there). Stimulus:

| tick | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| a | 1 | 1 | 1 | 0 | 1 | 1 | 0 | 0 |
| b | 0 | 1 | 1 | 1 | 0 | 1 | 1 | 0 |
| c | 1 | 0 | 1 | 1 | 1 | 0 | 1 | 0 |

- `m01` unequal-length branches `(a ##1 b) or (a ##2 c) or b`: `a`@1 `b`@2
  gives `[1,2]`; `a`@1 `c`@3 gives `[1,3]`; `b`@1 is 0. Ends {2, 3}.
- `m02` equal-length branches `(a ##1 b) or (a ##1 a)`: both end at 2, which
  is one match. Ends {2}.
- `m03` empty repetition on the left `a[*0:2] ##1 b`: the empty `a[*0]` makes
  it `b` at 1 (0); `a`@1 `b`@2 gives 2; `a`@1-2 `b`@3 gives 3. Ends {2, 3}.
- `m04` empty repetition on the right `c ##1 a[*0:1]`: `c`@1 with `a[*0]` is
  `c ##0 1`, ending at 1; with `a`@2 it ends at 2. Ends {1, 2}.
- `m05` `a[*0] ##0 c`: empty `##0` never matches. No lines.
- `m06` intersect endpoints `a[*1:3] intersect (c ##[0:2] c)`: the left ends
  at 1, 2, 3; the right ends at 1 (`##0`, `c`@1) and 3 (`##2`, `c`@3; `c`@2
  is 0). Common ends {1, 3}.
- `m07` first_match tie `first_match((a ##1 b) or (a ##1 a) or (c ##2 c))`:
  the first two branches both end at 2 and the third at 3; the earliest end is
  2, reported once. Ends {2}.
- `m08` `(a ##1 a) and (c ##2 c)`: left ends 2, right ends 3; the later is 3.
  Ends {3}.
- `m09` `(b ##1 b) within (a ##[1:3] c)`: the outer ends at 3 (`c`@3) and 4
  (`c`@4) (`c`@2 is 0). `b ##1 b` matches `[2,3]` and `[3,4]`; `[2,3]` lies in
  `[1,3]` and `[1,4]`. Ends {3, 4}.
- `m10` `a throughout (c ##[1:3] b)`: `c`@1, `b` at 2, 3, 4; `a` holds on
  1-3 but not on 4. Ends {2, 3}.
- `m11` `a[*0:1] and (c ##2 c)`: the left ends at 0 (empty) or 1, the right
  at 3, so the later end is 3 for both pairs, one match. Ends {3}.
- `m12` overlapping attempts of `a ##[1:2] b` on ticks 1, 2 and 3: start 1
  ends 2 and 3; start 2 ends 3 and 4; start 3 ends 4 (`b`@5 is 0). Lines
  2, 3, 3, 4, 4.

## A03 `unbounded`

`go` is high on tick 0 only; `a` is 1 on ticks 0-5 and 0 from tick 6; `b` is 1
on ticks 3 and 8 only; `c` is never 1. The run ends after tick 12. Passes are
printed by the `cover` twins (non-vacuous successes) and failures by the
`assert` twins' else actions.

- `u1` `go |-> ##[1:$] b`: first `b` after tick 0 is tick 3. Pass at 3.
- `u2` `go |-> b[->2]`: second `b` is tick 8. Pass at 8.
- `u3` `go |-> (a[*1:$] intersect (##[2:$] b))`: the left ends on ticks 0-5,
  the right on 3 and 8; first common end 3. Pass at 3.
- `u4` `go |-> (a[*1:$] intersect (##[4:$] b))`: the right can only end at
  8; the left dies on tick 6 (`a` is 0), after which no common end exists, so
  the obligation fails on tick 6 instead of pending until `b`.
- `u5` `go |-> ##[1:$] c`: never met; the weak obligation stays pending
  through the end of the run. No line (no false pass or fail).
- `u6` `go |-> (b[->1] and c[->1])`: the left matches on 3; the right never.
  Pending to the end. No line.
- `u7` `go |-> ((a[*1:$] ##1 !a) and (##[1:$] b))`: the left ends at 6, the
  right at 3 and 8; the earliest `and` end is max(6, 3) = 6. Pass at 6.
- `u8` `a |-> ##[1:$] b` with `a` high on ticks 0-5: attempts 0, 1, 2 pass
  at 3; attempts 3, 4, 5 pass at 8. Three lines at 3 and three at 8.

## A03 `budget`

`a |-> ##[1:$] b` with `a` always 1 and `b` never 1 adds pending threads on
every clock. With the default budget all 200 ticks run and `done t=200` is
printed (weak obligations pending at the end produce no result). With
`LLG_SEQUENCE_THREAD_LIMIT=50` the run stops with exit status 1 and
`llg: sequence thread budget exhausted: more than 50 live sequence threads
at time ...` naming assertion `p`, before `done` is printed; no attempt is
dropped silently. An invalid limit (`0`) is rejected at startup.

## Negatives

- `neg_join_local`: a local variable assignment inside an `and` operand is
  legal (16.10) but belongs to SIM-039; rejected with an explicit message.
- `neg_property_and`: `and` with an implication operand is property-level
  composition (SIM-038); rejected explicitly.
- `neg_multiclock_and`: an `and` operand with a different clock; Slang
  reports that the expression has no unique leading clock.
- `neg_goto_sequence`: goto repetition of a sequence is illegal (16.9.2).
- `neg_bad_clock_witness`: FND-002 witness `neg_assert_bad_clock`
  (L-F12-10-03): `intersect` of differently clocked sequences is illegal
  (16.13); adopted with its required diagnostic.
