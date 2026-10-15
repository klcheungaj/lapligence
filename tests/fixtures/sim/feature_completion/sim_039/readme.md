# SIM-039 fixtures: assertion locals, match items, end points and action controls

Run by [`tests/sim_feature_completion/sim_039.rs`](../../../../sim_feature_completion/sim_039.rs)
in both optimizer modes on the legacy, compact/portable and compact/GMP value
backends. Lines that share a time compare as a set: the LRM does not order
actions, match-item calls or processes of one region. Line numbers refer to
the `pdftotext -layout` extraction `SystemVerilog-1800-2009.txt` (SV). The
interpretations that needed a choice are the register decisions
[S39-D1 to S39-D9](../../../../../docs/lrm_decisions.md#assertion-locals-match-items-end-points-and-action-controls-sim-039).

## Clauses relied on

- 20.12, L35116-35118: "$assertpassoff shall stop execution of the pass action
  for vacuous and nonvacuous success of all the specified assertions.
  Execution of the pass action for both vacuous and nonvacuous successes can be
  re-enabled subsequently by $assertpasson, while the execution of pass action
  for only nonvacuous successes can be enabled subsequently by
  $assertnonvacuouson."
- 20.12, L35122-35123, L35135-35136: "$assertfailoff shall stop execution of the
  fail action of all the specified assertions until a subsequent
  $assertfailon. ... This task also affects the execution of default fail
  action block."
- 20.12, L35137-35138: "$assertvacuousoff shall stop execution of the pass
  action of all the specified assertions on vacuous success until a subsequent
  $assertpasson."
- 20.12, L35147-35148: "$assertnonvacuouson shall enable execution of the pass
  action of all the specified assertions on nonvacuous success."
- 20.12, L35153-35157: "When the system task is specified with arguments, the
  first argument indicates levels of the hierarchy ... Subsequent arguments
  specify which scopes of the model to control. These arguments can specify
  entire scopes (module, program, interface, always procedure, or initial
  procedure) or individual assertions."
- 20.12, L35160-35161: "These system tasks shall not affect the execution of
  pass or fail actions until the system task is executed. These system tasks
  shall not affect the statistics counters for the assertions."
- 20.11, L35048: "$assertoff shall stop the checking of all specified
  assertions until a subsequent $asserton."
- 9.4.2.4, L12000-12003: "the process executing the event control shall block
  until the specified sequence reaches its end point. A sequence reaches its
  end point whenever there is a match for the entire sequence. A process
  resumes execution following the Observed region in which the end point is
  detected."
- 9.4.4, L12076-12078: "The triggered status of a sequence is set during the
  Observed region and persists through the remainder of the time step (i.e.,
  until simulation time advances)."
- 16.14.5, L26207-26208: matched works "by storing the result of the source
  sequence match until the arrival of the first destination clock tick after
  the match."
- 16.8.2, L22313-22315: "At the completion of a match of the instance of the
  named sequence, the value of the inout or output local variable formal
  argument shall be cast to the type of and assigned to the local variable
  whose reference is the associated actual argument."
- 16.8.2, L22318-22320: "For each matching thread of the instance of the named
  sequence, at the completion of the match of that thread the value of the
  local variable formal argument in that thread shall be cast to the type of
  and assigned to the associated copy of the actual argument local variable."
- 16.10, L23800-23807: "At the beginning of each evaluation attempt of an
  instance of a named sequence or property, a new copy of each of its local
  variables shall be created and, if present, the corresponding initialization
  assignment shall be performed. Initialization assignments shall be performed
  in the Observed region in the order that they appear in the sequence or
  property declaration. For the purposes of this rule, all initialization
  assignments to local variable formal arguments shall be performed before any
  initialization assignment to a local variable declared in an
  assertion_variable_declaration. Non-local variables appearing in the
  expression of an initialization assignment to a local variable shall be
  evaluated using the Preponed values from the time slot in which the
  evaluation attempt begins."
- 16.11, L24187-24190: "All subroutine calls attached to a sequence are
  executed at every successful match of the sequence. For each successful
  match, the attached calls are executed in the order they appear in the list.
  Assertion evaluation does not wait on or receive data back from any attached
  subroutine. The subroutines are scheduled in the Reactive region, like an
  action block."
- 16.11, L24193-24195: "Actual argument expressions that are passed by value
  use sampled values of the underlying variables and are consistent with the
  variable values used to evaluate the sequence match."
- 16.15.3, L26585 onwards: `cover sequence` reports every match of an attempt
  (register decision AA-D3).

## action_controls

Ticks at 5, 15, ..., 85; `a |-> b` is vacuous while `a = 0`.

- 5: every assertion succeeds vacuously and every pass action runs.
- 15: `$assertvacuousoff(0, tb.u1)` hides u1's vacuous pass; u2 and top print.
- 25 (`a = b = 1`): `$assertpassoff(0, tb.u2.chk)` hides u2; u1 (nonvacuous),
  top and the cover print.
- 35 (`b = 0`): all fail; `$assertfailoff(0, top_chk, tb.u1.chk)` hides u1 and
  top; u2 prints its fail; `nd` has no action, so its default report prints.
- 45 (`b = 1`): `$assertnonvacuouson(0, tb.u2)` re-enables u2's nonvacuous
  pass; u1, u2, top and the cover print.
- 55 (`a = 0`): vacuous; u1 is still vacuous-off and u2 has only nonvacuous
  passes on (L35116-35118), so only top prints.
- 65: `$assertpasson`/`$assertfailon` clear everything; `$assertfailoff(0, nd)`
  and `$assertpassoff(0, cov)`; `a = 1, b = 0` fails u1, u2, top and `nd`
  (not reported).
- 75, 85: passes (nonvacuous, then vacuous) print for u1, u2 and top; the cover
  hit at 75 is not printed.
- Counters (L35161): eight failures (u1, u2, top and `nd` at 35, and again at
  65), three cover hits (25, 45, 75) and sixteen vacuous successes (four
  assertions at 5, 15, 55 and 85), exactly as without controls; one default
  failure report (`nd` at 35).

## action_procedural

- 0: `imm` and `dfr` fail and print; `nde` reports its default failure.
- 1: `$assertfailoff(0, check.imm, tb.check.dfr)`: both failures hidden; `nde`
  reports.
- 2: `$assertpassoff(0, tb)` selects every assertion in `tb`: no pass prints.
- 3: `$assertpasson(0, tb.check.imm)`: `imm` prints its pass; `dfr` stays off.
- 4: `$assertfailon` clears the fail rules, `$assertfailoff(0, check.nde)`;
  `imm` and `dfr` print their failures, `nde` is silent.
- 5: `imm` prints; `dfr`'s report was queued before `$assertfailoff(0,
  check.dfr)` in the same time step but executes after it in the Reactive
  region, so it is hidden (S39-D2).
- `e1` at 15 passes and prints; at 25 its pass action is off, at 35 its fail
  action is off (S39-D4); the process continues after every `expect`.
- Counters: only assertions without an else action count through the
  default failure path: `nde` fails at 0, 1, 4 and 5 (four, reported at 0 and
  1 only), and `e1` fails once at 35.

## selectors

Every assertion fails at each tick.

- 5: `ga`, `pa`, `ia` report.
- 15: `$assertoff(0, tb.g.ga, tb.blk.pa)` (a generate-block name and a named
  procedural block): only `ia`.
- 25: `$asserton(0, tb.g)` re-enables `ga`; `$assertfailoff(0, tb.probe.ia)`
  (an assertion in a task) hides `ia`.
- 35: `$asserton(0, tb.blk)` and `$assertfailon(0, tb.probe.ia)`: all three.

## endpoints_procedural

`abc` (posedge) matches at 35 (attempt 15) and 45 (attempt 25); `de`
(negedge) matches at 50 (`d` at 20, `e` at 50).

- P1 waits on `abc.triggered || de.triggered`; it wakes at 35 after the
  Observed region (L12000-12003) and reads `abc=1 de=0`; after `#0` it is
  still 35 (`abc=1`); at 36 the status has lapsed (`abc=0`, L12076-12078).
- P2 resumes on `@(abc)` at 35 and 45; its `wait (abc.triggered)` passes at
  once in the same time step.
- P3 reads in the Active region of 35 and 45, before Observed: 0 (S39-D7).
- P4 (program, Reactive region) reads 1 at 35 and 45.
- P5 resumes on `@(de)` at 50.

## endpoints_sequence

`e1` matches at 25, 35, 75 and 85.

- `t1`: `inst ##1 e1(...).triggered` on `clk`: `inst` at 15 and 65 gives 25 and
  75; `inst` at 45 has no match at 55.
- `pm`: `.matched` on `sysclk` (7, 21, 35, 49, 63, 77, 91): the 25 match is
  seen at 35 (also the 35 match, coincident, S39-D5), 75 at 77, 85 at 91.
- `m2`: `go` at 49, the first matched tick after is 77, `done` at 91.
- `e3`: `e1(...).triggered ##1 !proc2`: triggered at 35 and 85, `!proc2` at 45
  and 95.

## endpoints_dpi

`ab` matches at 25. The C companion calls the export twice per call. The
Active-region `always` reads 0 at 15 and 25; the process resumed by `@(ab)`
and the program read 1 at 25; one time unit later, 0.

## locals

Cycle n is the posedge at 10n+5: `a` in cycles 1-3, `b` in 2-4, `d` = 1, 2,
4, 8, 16 in cycles 1-5, `e` = 0, 3, 8, 0 in cycles 2-5.

- `top_acc`: `v = d` at the attempt start; `acc(v)` adds `d` twice and copies
  back (L22313-22315): attempt 1 `1+1+2 = 4` shown at 35, attempt 2 `2+2+4 = 8`
  at 45, attempt 3 `4+4+8 = 16` at 55.
- `top_pick` (cover sequence, every match): branch `(a, o = d)` gives `w = d`
  one cycle later (1 at 25, 2 at 35, 4 at 45); branch `b ##1 (1, o = d + 100)`
  assigns in the next cycle and shows one later (104 at 45, 108 at 55, 116 at
  65) (L22318-22320).
- `outer(d)`: `x = d` at the start, `chk(x + 1)` initializes `y` where `chk`
  starts: attempt 2 needs `e3 == 3`, hit at 35.
- `a ##1 chk(d)`: `y = d` of the cycle where `chk` starts: attempt 3 needs
  `e4 == 8`, hit at 45 (capturing at the attempt start would need 4).
- `decl`: `u = d`, then `w2 = u + 1` (L23800-23807): attempt 2 needs
  `e3 == 3`, hit at 35.
- `hold(d)`: `v` is the attempt-start `d`; `a |=> e == v + 1` fails at 25
  (`e2 = 0 != 2`) and 45 (`e4 = 8 != 5`), holds at 35 (S39-D9).
- `sel`: `s = 8'h80 | d[3:0]`: 81, 82, 84 one cycle after each `a`.

## match_items

- `rep`: `(a, x += d, note(1, x))[*3]` then `(b, note(2, x), x = x + 1,
  tnote(x))`. Attempt 1: `x` = 1, 3, 7 at 15, 25, 35, then `note 2 x=7` and
  `tnote x=8` at 45 (the arguments are taken at each call's place in the
  list, L24187-24188). Attempt 2: 2, 6 at 25, 35, then `a = 0` at 45 ends
  it. Attempt 3: 4 at 35. `calls` counts seven `note` calls (shown at 60).
- `cnt`: `$rose(a)` only at 15; `n` = 1, 2, 3 and `!a` at 45.
- `seen(q)`: `q` gains 1 at every posedge by a nonblocking assignment. The
  argument is the sampled `q` (1, 2, 3 at 15, 25, 35) and the body, run in the
  Reactive region (L24189-24190), reads the updated `q` (2, 3, 4) (S39-D8).

## match_native

`(a, x = x + d)[*2]` in cycles 1-2 gives 3 at 25 and in cycles 2-3 gives 6 at
35; the DPI-C import receives that attempt's `x` and the export reads the
current `d` (2 at 25, 4 at 35).

## Negative cases

| Fixture | Rejected by | Reason |
| --- | --- | --- |
| `neg_triggered_clock` | llg | `.triggered` in an assertion on a sequence ending on another clock (16.14.5) |
| `neg_matched_procedural` | Slang | `.matched` outside a sequence (16.14.6) |
| `neg_edge_sequence` | Slang | edge event control on a sequence (9.4.2.4) |
| `neg_endpoint_local` | Slang | a local variable is unassigned in the end point's own attempt (16.10) |
| `neg_inout_actual` | Slang | an output local formal bound to a non-local actual (16.8.2) |
| `neg_formal_type` | Slang | an `event` local formal (16.6.1) |
| `neg_empty_inout` | Slang | an inout local formal on a sequence admitting an empty match (16.8.2) |
| `neg_property_output` | Slang | an output local formal of a property (16.13.19) |
| `neg_match_call_output` | Slang | a match-item call with an output argument (16.11) |
| `neg_match_call_timing` | llg | a match-item task with a timing control: it would not run within the Reactive region |
| `neg_property_local` | llg | a property local read by a later property operand (`always`): unsupported form |
