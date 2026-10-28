# ADV-017 fixtures: assertion and coverage native API object lifecycles

Each `<name>.sv` has a C application `<name>.c` that registers the system
tasks the HDL calls. `tests/sim_feature_completion/adv_017.rs` loads it as a
run-time plugin (`LLG_VPI_PLUGIN`; on Windows a DLL linked against a model
import library) and, for `lifecycle`, also links it as a static archive with
`--dpi-lib`; it compares stdout with `<name>.out` in both optimizer modes on
the legacy, compact/portable and compact/GMP value backends. Clause numbers
are IEEE 1800-2009; line numbers refer to `SystemVerilog-1800-2009.txt`.

The expected outputs were derived by hand from the rules below and then
checked against a run; no line was accepted only because the run printed it.
Times are `vpiSimTime` ticks (1 ns). In every design `clk` starts at 0 and
toggles every 5 ns, so its rising edges are at 5, 15, 25, ...; stimulus
changes at even times, so a sampled value at an edge is the value set
before it.

## Rules the outputs rely on

- **Attempt callbacks.** 39.4.2 L65666: "cbAssertionStart. An assertion
  attempt has started."; L65670 "cbAssertionSuccess. An assertion attempt or
  property instance reaches a success state."; L65672
  "cbAssertionVacuousSuccess. An assertion attempt reaches a vacuous success
  state."; L65673 "cbAssertionDisabledEvaluation. An assertion attempt
  reaches the disabled state (e.g. as a result of disable iff condition
  becoming true ...)"; L65675 "cbAssertionFailure. An assertion attempt or a
  property fails to reach a success state or a sequence instance fails to
  match."; L65688 "cbAssertionKill. An attempt is killed (e.g., as a result
  of a control action, see 39.5.2)." So every attempt that starts reaches
  exactly one of success, vacuous success, failure, disabled or killed, or
  is still in progress at the end. A cover attempt that does not match
  runs no callback (a cover reports no failure; the 40.5.3 failure counter
  does not count it either, S46-D8); a `cover sequence` reports a success
  per match (L66654: "there can be multiple matches corresponding to a
  single attempt"). Decision A17-D1.
- **Attempt information.** L65735-L65736: "On disable, enable, reset, kill,
  pass action, fail action, vacuous action, and nonvacuous action callbacks,
  the returned p_vpi_attempt_info info pointer is NULL"; L65738 "On start
  and success callbacks, only the attemptStartTime field is valid."; L65777
  "the content of attemptStartTime is always the start time of the actual
  attempt". cb_time (L65756-L65765) is the start time for cbAssertionStart
  and the time of the event otherwise. failExpr is the property expression
  or immediate condition (decision A17-D4, see `relations` below).
- **Global clocking future functions.** 39.4.2.1 L65787-L65788: "The
  callback is executed at the nearest tick of the global clock strictly
  following the callback event"; "cb_time contains the time of the callback
  event".
- **Control.** 39.5.2 L65904 "vpiAssertionDisable disables the starting of
  any new attempts for this assertion. This has no effect on any existing
  attempts"; L65908 Enable; L65901 "vpiAssertionReset discards all current
  attempts in progress for this assertion and resets this assertion to its
  initial state."; L65940 "vpiAssertionKill discards the given attempt, but
  leaves the assertion enabled"; L65911-L65935 the pass, fail, vacuous and
  nonvacuous action operations, each with "no effect on any existing
  attempts". L65944 "vpiAssertionDisableStep disables step callbacks for
  this assertion. This has no effect if stepping is not enabled". 39.5.3
  L65977-L65979: a function that discards attempts "also ... flushes any
  pending instances that have not yet matured". vpi_control returns 1 on
  success and 0 on a failure (38.4 L61970). A control raises the
  callback of the same name with a NULL info pointer; discarded attempts
  each raise cbAssertionKill and count as killed (L66642, "killed (e.g. as a
  result of a control action, see 39.5.2)"). Decisions A17-D2, A17-D3.
- **System control.** 39.5.1 L65825-L65827: the handle is "a vpiHandle for a
  scope ... A NULL handle signifies that the control applies to all
  assertions regardless of scope."; L65845-L65857 SysOff, SysKill, SysOn,
  SysEnd ("All assertion callbacks currently installed shall be removed.
  Once this control is issued, no further assertion-related actions shall be
  permitted."). The system callbacks (39.4.1) run once per system control;
  cbAssertionSysInitialized and cbAssertionSysOn run after
  cbStartOfSimulation (L65579: "The assertion system can initialize before
  cbStartOfSimulation does or afterwards."). Decision A17-D3.
- **Counters.** 40.5.3 L66651: "in progress = attempts - (successes +
  vacuous success + disabled + killed + failures)".
- **Static information.** 39.3.2 L65565: "Assertion source information: the
  file, line, and column where the assertion is defined"; L65548 "Unnamed
  assertions cannot be found by name."; L65566 "Assertion clocking
  block/expression" is an expression, which llg does not expose
  (`LLG_VPI_UNSUPPORTED`).

## lifecycle (A01, A02)

`$attach` (time 0) lists the assertions in table order and places the six
attempt reasons on each; `a_req` is traced line by line, has a second
`cbAssertionStart` record and a `cbAssertionDisablePassAction` record.
Stimulus: `req` 1 at 2, 0 at 42, 1 at 52; `ack` 1 at 12, 0 at 22, 1 at 52;
`rst` 1 from 28 to 32. Sampled at the edges 5..65: `req` 1 1 1 1 0 1 1,
`ack` 0 1 0 0 0 1 1.

- `a_req` (`disable iff (rst) req |=> ack`): attempt 5 succeeds at 15
  (ack 1); 15 fails at 25; 25 is in progress when `rst` rises at 28, so it
  is disabled at 28; 35 is killed by `$ctl(1)` at 38 (cb_time 38, info
  NULL); `$ctl(2)` at 44 disables the pass action (callback at 44); 45 has
  `req` 0, a vacuous success; 55 succeeds at 65; 65 is in progress. Tally:
  start 7, success 2, vacuous 1, failure 1, disabled 1, kill 1, one in
  progress; the counters agree. Pass actions ran only for the success at
  15 (`a_pass` 1); the fail action ran for 25 (`a_fail` 1). The vacuous
  success at 45 and the success at 65 ran no action yet did raise their
  callbacks: a disabled action and a killed attempt are told apart.
- The second Start record runs at 5, 15, 25 and 35 and is removed at 38: 4
  calls. `a_req`'s own handle is released at 38; the callbacks keep theirs.
- `m_two` (`req |=> @(posedge clk2) ack`, `clk2` edges 3, 9, 15, 21, 27,
  33, 39, 45, 51, 57, 63, 69): from 5 the consequent is due at 9 (ack 0,
  failure), 15 at 21 (success), 25 at 27 (failure), 35 at 39 (failure), 45
  is vacuous, 55 at 57 (success), 65 at 69 (success). Tally 7/3/1/3, none in
  progress; pass actions 4 (3 successes and the vacuous success run the
  pass action), fail actions 3.
- `f_fut` (`$rose(req) |-> $future_gclk(req)`, global clock edges at every
  odd time): `$rose` holds at 5 (from the initial 0) and at 55; `req` is 1
  at 7 and 57, so both succeed; the other five attempts are vacuous. Each
  result is delivered at the next global clock tick, before `$report`.
  Tally 7/2/5/0, pass actions 7.
- `k_cov` (`cover property (req ##1 ack)`): 5 matches at 15, 55 at 65;
  15, 25 and 35 fail at the next edge (ack 0) and 45 at once (req 0); 65 is
  in progress. The four attempts without a match run no callback, so the
  tally is 7/2/0/0 with 5 not concluded; the failure counter stays 0
  (covers); `cov_hits` 2.
- `tb.u_hs.c_ack` and `tb.u_hs.c_req`, owned by the checker instance, see the
  same sampled values as `a_req` without its reset and controls: `c_ack`
  succeeds at 15 and 65, fails at 25, 35 and 45 (from 15, 25, 35), is vacuous
  at 45, one in progress; `c_req` is as `k_cov`.

## control (A03)

`registration` places a record for each of the 12 assertion system reasons,
so `SysInitialized` and `SysOn` run at 0 after the start of simulation.
`$setup` places Start, Success, Failure and Kill on `p`, `im`, `df` and
`u_sub.s_x` (only `p` traced; the others are tallied) and the 9 control
reasons on `p`, `im` and `df`. `vpi_get_cb_info` of an assertion callback
gives its reason and assertion, with a NULL `cb_rtn` (the routine has the
39.4.2 signature, not `cb_rtn`'s). A late `cbAssertionSysInitialized` is
accepted and never runs (as S43-D4); a step callback is accepted and never
runs, as stepping cannot be enabled (A17-D2). A module handle, a NULL
routine and a time reason fail with level 3.

`a` is 1 throughout; `b` is 0 until 50, 1 from 50 to 70, 0 from 70 to 90,
then 1, so sampled `b` at 5..115 is 0 0 0 0 0 1 1 0 0 1 1 1.

| time | step | effect on `p` (`a |=> b`) |
| --- | --- | --- |
| 5 | | start 5 |
| 8 | 1 | Disable on p, im, df; NULL and module handles fail |
| 15 | | 5 fails (fail action: `p_fail` 1); no new attempt |
| 28 | 2 | Enable on p, im, df |
| 35 | | start 35 |
| 38 | 3 | kill 35: Kill; kill 7 (no such attempt) returns 0 with level 3; `vpiSuppressTime` fails |
| 45 | | start 45 |
| 48 | 4 | Reset: 45 discarded (counted as killed, no Kill callback), then Reset |
| 55 | | start 55 |
| 58 | 5 | p fail action off, im pass action off, p vacuous action off |
| 65 | | 55 succeeds (`p_pass` 1); start 65 |
| 75 | | 65 fails, fail action off; start 75 |
| 78 | 6 | p fail action on, im pass action on, p nonvacuous action on |
| 85 | | 75 fails (`p_fail` 2); start 85 |
| 88 | 7 | SysOff on `tb.u_sub` (only `s_x`): SysOff |
| 95 | | 85 succeeds (`p_pass` 2); start 95 |
| 98 | 8 | SysOn (NULL); SysKill on `p`: Kill 95, p disabled; SysReset on `tb`: everything enabled |
| 105 | | start 105 |
| 108 | 9 | the six system action operations, one callback each |
| 115 | | 105 succeeds (`p_pass` 3); start 115 |
| 118 | 10 | EnableStep and DisableStep fail; SysEnd with a scope fails; SysEnd: Kill 115, SysEnd; then controls and registrations fail |
| 122 | | `$asserton` cannot restart anything |

`p`: attempts 10, success 3, failure 3, killed 4. `im` (`assert (b)` at each
edge while enabled: 5, 35..115) has 10 attempts, 5 successes (55, 65, 95,
105, 115), 5 failures; its pass action was off at 65, so `im_pass` is 4 and
`im_fail` 5. `df` (`assert #0 (b)`) has the same outcomes with every action
running: 5 and 5. `s_x` misses only the edge at 95 and stops after 118:
11 attempts, 4 successes (55, 65, 105, 115), 7 failures.

## objects (39.3)

`vpi_iterate(vpiAssertion, NULL)` lists the items in table order with type,
name, full name, file, line and column of the statement (the label for a
labelled one), `vpiIsDeferred`, the owning instance and its definition. The
unnamed immediate assertion in `leaf` has no name or full name. Iterating
from `tb.u_leaf` and from the checker instance `tb.u_chk` gives their own
assertions; `k_x` is found by name below `tb.u_chk`. The model is built
without `--vpi`, so it has no expression objects: `vpiClockingEvent`,
`vpiDisableCondition` and `vpiProperty` fail with `LLG_VPI_UNSUPPORTED`
(`relations` covers them with `--vpi`). `vpiEndLine` of `r1` is 24: the
statement ends on its own line (37.45, A17-D4).

`r1` (`v` is always 1) has three Start records: a trace, a `killer` that
kills the attempt starting at 15 from inside its own Start callback, and a
`remover` that removes itself on its second call. The kill is delivered
inside the killer's call, before the next Start record runs; the attempt
from 15 never succeeds; the remover runs at 5 and 15 only. Attempts 5, 25
succeed at 15, 35; 35 is in progress: attempts 4, success 2, killed 1;
`vpiCovered` is 1 (attempted, succeeded, never failed, 40.5.3 L66606).

## relations (37.45-37.51, 39.4.2; ADV-017b)

Built with `--vpi`, so the model has expression objects. `$relations`
prints, for each assertion in table order, its type, full name, `vpiEndLine`
and `vpiEndColumn`, `vpiIsClockInferred` and `vpiIsCoverSequence` (-1 where
the property does not apply: immediate assertions have no clock, only a
cover is a cover sequence), then each one-to-one relation of 37.46/37.51.
`decompile=NULL` marks an object without `vpiDecompile` (specifications,
instances, statements without a decompiled form, opaque operators).

- **End position.** 37.45 lists `vpiEndLine` and `vpiEndColumn` under
  "location" without defining them; llg gives the statement's last
  character, `;` included (A17-D4). `a_spec` ends with the `;` at column 43
  of line 32; `a_inst` at 33:39; `i1` at 41:64 (`... $time);`).
- **Clocking event.** 37.46 detail 1 (L60456-L60457): "Clocking event is
  always the actual clocking event on which the assertion is being
  evaluated, regardless of whether this is explicit or implicit
  (inferred)." `a_spec`, `c_seq`, `a_bool` write theirs (`posedge clk`,
  `posedge clk iff a`); `a_inst` and `a_nest` take the clock written in
  `p1` (line 23) and `p2` (line 26); `a_proc` takes the `negedge clk` of its
  procedure (line 37, 16.15.6) and `l_def` the default clocking of `leaf`
  (line 10): those two are inferred (1).
- **Disable condition.** Only the `disable iff` written in the statement
  (A17-D4; the text is silent): `a_spec` writes `rst`; the `disable iff
  (rst)` inside `p1` (`a_inst`) and the `default disable iff (!a)` of
  `leaf` (`l_def`) are not reported; the others have none (NULL, no
  error).
- **Property.** 37.46 shows `vpiProperty` reaching a property instance or a
  property specification: `a_inst` and `a_nest` instantiate exactly one
  named property and get the `vpiPropertyInst`; every other statement gets a
  `vpiPropertySpec` (37.48) with the clocking event and disable condition
  written in the statement and its `vpiPropertyExpr`: the Boolean `b` and
  `a` are the variables themselves, `s1(a, 1)` is a `vpiSequenceInst`, and
  the operators `a |=> b`, `a ##1 b` are opaque `vpiOperation` objects with
  no decompile and no line (A17-D4).
- **Statements.** 37.46 and 37.51 give `vpiStmt` and `vpiElseStmt`. An
  empty action `;` and a missing else action are NULL (A17-D4); `a_spec`
  has both `$display` actions, `i1` only its else `$display`, `i2` its pass
  `$display`. Immediate assertions have `vpiExpr` (`a == b`, `a`);
  concurrent ones have none (NULL).
- **Instances.** 37.50 detail 1 (L60641-L60644) and 37.52 detail 1
  (L60739-L60742): "The vpiArgument iterator shall return the ... instance
  arguments in the order that the formals ... are declared ... If a formal
  has a default value, that value shall appear as the argument should the
  instantiation not provide a value". `p1(a, b)` gives `a`, `b`;
  `s1(a, 1)` gives `a` and the literal 1 (line 34); the nested `s1(a)` of
  `a_nest` gives `a` and the default 2 (line 19). The declaration (37.47,
  37.49) is named `tb.p1`, `tb.s1`, `tb.p2` at its `property`/`sequence`
  line; its formals iterate in declaration order (detail 1). Detail 4:
  "vpiDirection returns vpiNoDirection if the formal argument is not a
  local variable argument" (5); `local input logic y` is `vpiInput` (1).
  Detail 2: "The vpiTypeSpec relation shall return NULL if the formal is
  untyped": `x` and `s` give NULL, `int n` a `vpiIntTypespec`, `logic y` a
  `vpiLogicTypespec`. Detail 3's `vpiExpr` (a formal's default), and a
  declaration's `vpiVariables`, are not kept: `LLG_VPI_UNSUPPORTED`.
- **Declarations of a scope.** 37.12 lists property and sequence
  declarations of a scope: `tb` declares `tb.s1`, then `tb.p1` and `tb.p2`;
  `tb.u_leaf` declares none (NULL iterator, no error).
- **Instance callbacks.** 39.4.2 L65702-L65704: "The cbAssertionStart,
  cbAssertionSuccess, and cbAssertionFailure callbacks may also be
  registered on a sequence instance or a property instance." Placing
  `cbAssertionVacuousSuccess` fails (`LLG_VPI_CALLBACK`); 39.5.2
  L65886-L65887: "Only assertion statement handles are valid here, not
  sequence or property instances" (`vpiAssertionDisable` on an instance:
  `LLG_VPI_ARGUMENT`). The nested `s1(a)` of `a_nest` refuses a callback
  (`LLG_VPI_UNSUPPORTED`, A17-D5). `p1` of `a_inst` starts at every
  posedge (5, 15, 25) and succeeds at 25 for attempt 5 (`a` at 5, `b` at 25,
  `y` = `b` sampled at 5); `s1(a, 1)` of `c_seq` (`a ##1 b`) starts at 5,
  15, 25 and matches at 25 for attempt 15; attempt 5 misses (`b` is 0 at 15)
  and a cover sequence reports no failure (A17-D1, A17-D5).
- **failExpr.** 39.4.2 L65739: "On a cbAssertionFailure callback, the
  attemptStartTime and detail.failExpr fields are valid." llg passes the
  property expression (A17-D4): `i1` fails at 15 (`a` 1, `b` 0) with
  `a == b`; `a_spec` attempt 5 fails at 15 (`b` 0 at 15) with the opaque
  `a |=> b`. The run ends at 32, so `a_inst` attempt 15, whose `s1(a)`
  needs `b` at 35, is still in progress.
