# ADV-013: multiclock assertions, clock resolution and recursion

These public HDL fixtures cover multiclocked sequences and properties (IEEE
1800-2009 16.14), clock resolution (16.17), the clock-resolution functions
`$global_clock` (14.14), `$inferred_clock`/`$inferred_disable`
(16.15.7) and recursive properties (16.13.17, Annex F.7) through
[`sim_feature_completion::adv_013`](../../../../sim_feature_completion/adv_013.rs).
Positive cases run in both optimizer modes on the legacy, compact/portable and
compact/GMP value backends. Action lines of different assertions in one time
step have no defined order, so each `.out` file is compared as a set of lines.
`clocks`, `domains`, `flow` and `recursion` declare `` `timescale 1ns/1ps ``
so that the printed times do not depend on a tool's default time unit.
Every expected line was derived by hand from the stimulus and the clauses
quoted below, not copied from a simulator transcript. Line numbers refer to
the `pdftotext -layout` extraction `SystemVerilog-1800-2009.txt`.

Run the suite with:

```sh
scripts/run-tests.sh -E 'binary(sim_feature_completion) & test(adv_013::)'
```

## Clauses the expectations rely on

Multiclocked concatenation (16.14.1, L25857-25862):

> Multiclocked sequences are built by concatenating singly clocked subsequences using the single-delay
> concatenation operator ##1 or the zero-delay concatenation operator ##0. The single delay indicated by ##1
> is understood to be from the end point of the first sequence, which occurs at a tick of the first clock, to the
> nearest strictly subsequent tick of the second clock, where the second sequence begins. The zero delay
> indicated by ##0 is understood to be from the end point of the first sequence, which occurs at a tick of the
> first clock, to the nearest possibly overlapping tick of the second clock, where the second sequence begins.

Clock flow (16.14.3, L26026-26029, L26070-26072, L26099):

> Intuitively, clock flow provides that in a multiclocked sequence or property, the scope of a clocking event
> flows left to right across linear operators (e.g., repetition, concatenation, negation, implication, followed-by,
> and the nexttime, always, eventually operators) and distributes to the operands of branching operators
> (e.g., conjunction, disjunction, intersection, if–else, and the until operators) until it is replaced by a new

> sequence, also flows left to right across the parenthesized subexpression. However, the scope of a clocking
> event does not flow out of enclosing parentheses.

> Juxtaposing two clocking events nullifies the first of them; therefore, the following two-clocking-event

Annex F.5.1 rewrites the consequent of an implication under the clock that
flows into the implication (`T_p((r |-> p), c)` rewrites `p` under `c`).
llg follows the left-to-right flow of 16.14.3 instead (decision A13-D1,
industry-standard behaviour): the consequent of `|->` and `|=>` takes the
clock the antecedent ends on, unless that clock is inside parentheses or a
sequence instance. Both branches of a multiclocked `if`-`else` start at the
tick that reads the condition (decision A13-D3).

Clock ticks (16.5, L21468-21469):

> A clock tick is an atomic moment in time that itself spans no duration of time. A clock shall tick only once at
> any simulation time, and the sampled values for that simulation time are used for evaluation of concurrent

Clocking-block events (14.13, L19763-19764):

> The clocking event of a clocking block is available directly by using the clocking block name, regardless of
> the actual clocking event used to declare the clocking block.

Global clocking (14.14, L19973-19976):

> The system function $global_clock returns the event expression specified in the unique
> global clocking declaration. The function has no arguments. It shall be an error to invoke the
> $global_clock system function if there is no global clocking declaration in the elaborated SystemVerilog
> model. Otherwise, $global_clock may be invoked anywhere that a clocking event may be specified.

Inferred value functions (16.15.7, L27194-27197, L27199-27201, L27206-27211):

> The inferred clocking event expression is the current resolved event expression that can be used in a
> clocking event definition. It is obtained by applying clock flow rules to the point where $inferred_clock
> is called. If there is no current resolved event expression when $inferred_clock is encountered then an
> error shall be issued.

> The inferred disable expression is the disable condition from the default disable declaration whose scope
> includes the call to $inferred_disable (see 16.16). If the call to $inferred_disable is not within the
> scope of any default disable declaration, then the call to $inferred_disable returns 1'b0 (false).

> is used as the entire default value expression for a formal argument to a property or sequence declaration,
> then it is replaced by the inferred expression as determined at the point where the property or sequence is
> instantiated. Therefore, if the property or sequence instance is the top-level property expression in an
> assertion statement, the event expression that is used to replace the default argument $inferred_clock is
> that as determined at the location of the assertion statement. If the property or sequence instance is not the
> top-level property expression in the assertion statement then the event expression determined by clock flow

Semantic leading clocks (16.17.1, L27841-27842): "If inherited is the
semantic leading clock of m, then the semantic leading clock of @(c) m is c.
Otherwise, the semantic leading clock of @(c) m is equal to the semantic
leading clock of m."

Recursive properties (16.13.17, L25430-25431):

> SystemVerilog allows recursive properties. A named property is recursive if its declaration involves an
> instantiation of itself. Recursion provides a flexible framework for coding properties to serve as ongoing

Their meaning is the limit of the k-fold approximations (F.7, L71669-71675,
L71688-71689): `p[0]` replaces the body by `1'b1`, `p[k]` replaces each
recursive instance by its `(k - 1)`-fold approximation, and "w, L ⊨ π iff for
all k > 0, w, L ⊨ π[k]". A recursive obligation therefore never fails by
running out of trace: it is weak, and an attempt whose recursion is still
open at `$finish` reports nothing.

Synchronous aborts (16.13.14, L25269-25271):

> The operators sync_accept_on and sync_reject_on are evaluated at the simulation time step when the
> clocking event happens, unlike disable iff, accept_on and reject_on. Their abort condition is
> evaluated using sampled value as for accept_on and reject_on. The operators sync_accept_on and

Nested aborts (16.13.14, L25335-25337): "if two nested operator conditions
become true in the same time step during the evaluation of the argument
property, then the outermost operator takes precedence."

## Stimulus of `flow.sv`, `clocks.sv`, `recursion.sv` and `domains.sv`

The fixtures use one race-free stimulus: clocks change on multiples of five,
data at other times, so every sampled value is the last change before the
tick. `$finish` is at 115.

| Clock | Rising | Falling |
| --- | --- | --- |
| `c1` | 10, 30, 50, 70, 90 | 15, 35, 55, 75, 95 |
| `c2` | 20, 30, 45, 60, 90, 100 | 25, 33, 48, 63, 92, 102 |

`a` is 1 from 1, 0 from 12, 1 from 22, 0 from 41, 1 from 61; `b` is 1 from 2,
0 from 21, 1 from 31, 0 from 51, 1 from 58, 0 from 91; `d` is 1 from 3, 0 from
44, 1 from 66; `e` is 1 from 26 to 36; `rst` (clocks.sv) is 1 from 40 to 42.
Sampled values at the ticks:

| t | 10 | 15 | 20 | 25 | 30 | 33 | 35 | 45 | 48 | 50 | 55 | 60 | 63 | 70 | 75 | 90 | 92 | 95 | 100 | 102 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| a | 1 | 0 | 0 | 1 | 1 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 |
| b | 1 | 1 | 1 | 0 | 0 | 1 | 1 | 1 | 1 | 1 | 0 | 1 | 1 | 1 | 1 | 1 | 0 | 0 | 0 | 0 |
| d | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 1 | 1 | 1 | 1 | 1 | 1 | 1 |
| e | 0 | 0 | 0 | 0 | 1 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |

A pass with a false antecedent is a vacuous success, which runs the pass
action. An obligation still open at `$finish` reports nothing unless it is an
`s_eventually` (S38-D1, A13-D7).

## `flow.sv`

Attempts start at the `posedge c1` ticks 10, 30, 50, 70, 90 unless noted.

- `f1` `a ##1 @(posedge c2) b`: b at the first `c2` tick strictly after the
  attempt: 10→20 (1), 30→45 (1), 50 a=0, 70→90 (1), 90→100 (0). P20 P45 F50
  P90 F100.
- `f2` `a ##0 @(posedge c2) b`: the first `c2` tick at or after: 10→20 (1),
  30→30 (0), 50 a=0, 70→90 (1), 90→90 (1). P20 F30 F50 P90 P90.
- `f3` `a |-> @(posedge c2) b`: as `f2`, with 50 vacuous. P20 F30 P50 P90 P90.
- `f4` `a |=> @(posedge c2) b`: as `f1`, with 50 vacuous. P20 P45 P50 P90 F100.
- `f5`, `f5r` (16.14.3 L26033-26039: `@(c) x |=> y ##1 @(d) z` is
  `@(c) x |=> @(c) y ##1 @(d) z`): d at the next `c1` tick, then b at the next
  `c2` tick: 10→d30=1→b45=1 P45; 30→d50=0 F50; 50 vacuous P50; 70→d90=1→
  b100=0 F100; 90 has no later `c1` tick (open). Both: P45 F50 P50 F100.
- `f6`, `f6r` (adjointness, `@(c) x ##1 y |=> @(d) z` is
  `@(c) x |=> y |=> @(d) z`): 10: a, d30=1 → b45=1 P45; 30: d50=0, vacuous P50;
  50 vacuous P50; 70: d90=1 → b100=0 F100; 90 open. Both: P45 P50 P50 F100.
- `f7`, `f7r` (`@(c) x ##0 y |=> @(d) z` is `@(c) x |-> y |=> @(d) z`): a and
  d at the attempt, then b at the next `c2` tick: 10→20 P20, 30→45 P45, 50
  vacuous P50, 70→90 P90, 90→100 F100.
- `f8` (16.14.3 L26075-26079, `@(c) w ##1 (x ##1 @(d) y) |=> z`: z is clocked
  by c): 10: d30=1, b45=1, e at the next `c1` tick 50 is 0: F50; 30: d50=0
  vacuous P50; 50 vacuous P50; 70: d90=1, b100=0 vacuous P100; 90 open.
- `f9` (16.14.3 L26082-26090, `@(c) v |=> (w ##1 @(d) x) and (y ##1 z)`):
  10: right b30=0 fails at 30 F30; 30: left d50=0 fails F50; 50 vacuous P50;
  70: left d90=1, b100=0 fails F100; 90 open.
- `f10` `@(posedge c2) @(posedge c1) a` is `@(posedge c1) a` (juxtaposition;
  the leading clock of `@(c) m` is that of `m`): P10 P30 F50 P70 P90.
- `f11` `@(posedge c1) s2` with `s2` clocked by `posedge c2`: the instance's
  clock is its semantic leading clock, so attempts are the `c2` ticks: b at
  20, 30, 45, 60, 90, 100 is 1, 0, 1, 1, 1, 0. P20 F30 P45 P60 P90 F100.
- `f12` `a ##1 (@(posedge c2) b) ##1 d`: c1 flows across the parentheses, so
  d is at the first `c1` tick after b: 10→b20→d30=1 P30; 30→b45→d50=0 F50; 50
  F50; 70→b90→ no later `c1` tick (open); 90→b100=0 F100.
- `f13` `a ##1 @(posedge c2) b |=> d`: the consequent is clocked by c2, the
  antecedent's last clock (A13-D1), so d is at the next `c2` tick after b:
  10→b20→d30=1 P30; 30→b45→d60=0 F60; 50 vacuous P50; 70→b90→d100=1 P100;
  90→b100=0 vacuous P100.

## `clocks.sv`

- `k1` `@(c1) a`, `k2` `@(edge c1) a`: both edges of c1 are ticks: a at 10,
  15, 30, 35, 50, 55, 70, 75, 90, 95 is 1, 0, 1, 1, 0, 0, 1, 1, 1, 1.
- `k3` `@(posedge c1 or posedge c2) a`, and `k9` (default clocking of `dmod`
  with the same event list): ticks 10, 20, 30, 45, 50, 60, 70, 90, 100. The
  coincident edges at 30 and 90 are one tick each (16.5). a: 1, 0, 1, 0, 0,
  0, 1, 1, 1.
- `k4` `@(cb) b` with `cb` clocked by `posedge c2` (14.13): b at the `c2`
  ticks: P20 F30 P45 P60 P90 F100.
- `k5` `@(posedge cv[1]) d` with `cv = {c2, c1}`: d at the `c2` ticks: P20
  P30 F45 F60 P90 P100.
- `k6` `a ##1 @(posedge c2 iff d) b`: the gated clock ticks where d holds at
  the edge: 20, 30, 90, 100. 10→20 b=1 P20; 30→90 P90; 50 a=0 F50; 70→90 P90;
  90→100 b=0 F100.
- `k7` `@(posedge c1 iff d) a ##1 @(posedge c1) b`: attempts where d holds:
  10, 30, 70, 90; the second clock is ungated: 10→b30=0 F30; 30→b50=1 P50;
  70→b90=1 P90; 90 open.
- `k8` `a |=> @(c2) b`: c2 changes at 20, 25, 30, 33, 45, 48, 60, 63, 90, 92,
  100, 102: 10→20 P20; 30→33 P33; 50 vacuous P50; 70→90 P90; 90→92 b=0 F92.
- `g1` `@($global_clock) a |=> b`, global clocking on `posedge c1 or negedge
  c2`: ticks 10, 25, 30, 33, 48, 50, 63, 70, 90, 92, 102. 10→b25=0 F25; 25→
  b30=0 F30; 30→b33=1 P33; 33→b48=1 P48; 48 and 50 vacuous P48 P50; 63→b70=1
  P70; 70→b90=1 P90; 90→b92=0 F92; 92→b102=0 F102; 102 open.
- `g3` `repeat (4) @($global_clock)`: the first four global ticks, 10 25 30
  33.
- `i1` `q(a)` in `imod_c2`, default clocking `posedge c2`, `q(x, clk =
  $inferred_clock)` is `@clk x |=> b`: the top-level instance takes the
  default clock: 20 vacuous P20; 30→b45=1 P45; 45 and 60 vacuous P45 P60;
  90→b100=0 F100; 100 open.
- `i6` the same property in `always @(posedge c2)`: the procedure's clock is
  the inferred clock (16.17 c)), so the lines are those of `i1`.
- `i2` the 16.15.7 example `p_multiclock(negedge c2, , posedge c1, a, b, d,
  e)` under default clocking `negedge c1` is `@(negedge c2) a ##1 @(negedge
  c1) b |=> @(posedge c1) d ##1 e`: attempts at 25, 33, 48, 63, 92, 102.
  25→b35=1→d50=0 F50; 33→b35→d50 F50; 48 vacuous P48; 63→b75=1→d90=1→ no
  later `c1` tick (open); 92→b95=0 vacuous P95; 102 open.
- `i3` `a |=> @(posedge c2) q2(b)` in `imod_c1` (default clocking `posedge
  c1`, default disable `rst`), `q2(x, clk = $inferred_clock)` is `@clk x`:
  the instance is not top-level, so its clock is the flowing `posedge c2`.
  10→20 P20; 30's attempt is disabled by rst at 40; 50 vacuous P50; 70→90
  P90; 90→100 F100.
- `i4` `p(a)` with `p(x, r = $inferred_disable)` = `disable iff (r) x |=>
  b`: r is the default disable `rst`: 10→b30=0 F30; 30 disabled at 40; 50
  vacuous P50; 70→b90=1 P90; 90 open.
- `i5` `p5(a)` in `tb`, which has no default disable: r is 1'b0, so nothing
  is disabled: F30, then 30→b50=1 P50, 50 vacuous P50, 70→ P90.

## `restrict.sv`

16.15.4 (L26658-26659): "the restrict property statement is not verified in
simulation and has no action block". Module-level, generate, `initial` and
procedural `restrict property` statements whose properties fail produce no
output and no assertion; the ordinary `a1` beside them passes. Industry-standard
behaviour prints only `done n=2` for the same design without the checker
(probed 2026-11-12; the probed tool rejected the checker declaration). llg reports
each statement once as `ELAB-005` INFO: six reports for the six statements
(module `sub` instantiated twice, checker `chk`, module item, `initial`,
procedural, and the two-iteration generate loop).

## `mc_segment.sv`

16.14.1 (L25903-25904): "When concatenating differently clocked sequences,
the maximal singly clocked subsequences are required to admit only nonempty
matches." The subsequence before `@(posedge c2)` is `a ##1 b[*0:1]`, which
cannot match empty, so the form is legal; only a subsequence whose every
element can match empty is rejected (`neg_mc_empty.sv`). `c1` rises at 5,
15, ..., 95 and `c2` at 7, 21, ..., 91; a and d stay 1 and b is 1 from 52.
Each `c1` attempt from 5 to 85 matches through the `b[*0]` branch
(`a ##0 1`, 16.9.2.1), the attempt at 95 has no later `c2` tick (n1=9); the
attempts from 45 to 75 also match with b sampled 1 at the next `c1` tick
(n2=4).

## `recursion.sv`

All assertions are clocked by `posedge c1` (ticks 10, 30, 50, 70, 90, with no
`c1` tick after 90). Sampled values at those ticks: a 1 1 0 1 1, b 1 0 1 1 1,
d 1 1 0 1 1, e 0 1 0 0 0; at the `c2` ticks 20, 30, 45, 60, 90, 100: d 1 1 0
0 1 1.

- `r1` `a |-> prop_always(a)` (16.13.17 L25436-25438, and SIM-038's former
  `neg_recursive_property`): a at every tick from the attempt: 10 and 30 fail
  at a50=0 (F50 F50); 50 vacuous P50; 70 and 90 open.
- `r2` `a |=> prop_always(d)`: d at every tick after the attempt: 10→d30=1,
  d50=0 F50; 30→d50=0 F50; 50 vacuous P50; 70→d90=1 then open; 90 open.
- `r3` `prop_weak_until(d, e)` (L25450-25452, `q or (p and (1'b1 |=>
  ...))`): 10: e=0, d=1, then e30=1 P30; 30: e=1 P30; 50: e=0 and d=0 F50;
  70 and 90: d holds and e never does, open.
- `r4` the mutual recursion `check_phase1`/`check_phase2` (L25465-25470) with
  `s1 = a`, `s2 = b`, both phase properties `d`: 10: a, d10, then phase 2 at
  30 with b30=0 vacuous P30; 30: a, d30, then b50=1 and d50=0 F50; 50 vacuous
  P50; 70: phase 2 at 90 (b, d) then open; 90 open.
- `r5` `a |=> swap(b, d)`, `swap(x, y) = x and (1'b1 |=> swap(y, x))`
  (swapped formals, restriction 4 first form): b, d, b, ... from the next
  tick: 10→b30=0 F30; 30→b50=1, d70=1, b90=1, open; 50 vacuous P50; 70→b90=1
  open; 90 open.
- `r6` `a |=> then_d(e)`, `then_d(x) = x and (1'b1 |=> then_d(d))` (a
  constant actual, restriction 4 second form): e at the next tick, then d at
  every later tick: 10→e30=1, d50=0 F50; 30→e50=0 F50; 50 vacuous P50;
  70→e90=0 F90; 90 open.
- `r7` `two_sites(e, b)`, `(x |=> (y and two_sites(x, y))) and (y or
  (1'b1 |=> two_sites(x, y)))`: both conjuncts are done at a tick where x is 0
  and y is 1; where x is 1 the next tick needs y, and where y is 0 the
  property recurses: 10 (e=0, b=1) P10; 30 (e=1, b=0) needs b50=1 and recurses
  at both sites into 50, where e=0 and b=1: P50; 50 P50; 70 P70; 90 P90. (The
  form without `y and` could never fail, so its attempts would be decided at
  once by any extension argument; this one is not.)
- `r8` the 16.13.17 abort example (L25570-25571) in its synchronous form:
  `a |-> sync_p3(d, b, e)`, `sync_p4 = sync_accept_on(bb)
  sync_reject_on(abort) sync_p3(...)`: at the attempt d must hold, then at
  each next tick the outer accept on b wins over the inner reject on e: 10:
  d10=1, at 30 b=0 and e=1 reject F30; 30: d30=1, at 50 b=1 accept P50; 50
  vacuous P50; 70: d70=1, at 90 b=1 accept P90; 90: d90=1, no next tick, open.
- `r9` `a |=> @(posedge c2) prop_always(d)`: d at every `c2` tick strictly
  after the attempt: 10→d20=1, d30=1, d45=0 F45; 30→d45=0 F45; 50 vacuous
  P50; 70→d90, d100 then open; 90→d100 open.
- `r10` `a |=> on_c2(d)`, `on_c2(x) = x and (1'b1 |=> @(posedge c2)
  on_c2(x))`: the first instance is clocked by c1 and the recursion by c2: d
  at the next `c1` tick, then at every later `c2` tick: 10→d30=1, d45=0 F45;
  30→d50=0 F50; 50 vacuous P50; 70→d90=1, d100=1 open; 90 open.
- `r11` the F.7 dependency example `p1`/`p2`/`p3` (L71630-71640) as
  `p1(d)`: `p1(v) = v |=> p2(p3())`, `p2(v) = a or (1'b1 |=> v)`, `p3 =
  p1(a && b)`. 10: d, then a30=1 P30; 30: d, a50=0, then `p1(a && b)` at 70
  with a&&b=1 and a90=1 P90; 50: d=0 vacuous P50; 70: d, a90=1 P90; 90 open.

## `domains.sv`

Additional stimulus: `x` is 1 from 1, 2 from 22, 3 from 41, 4 from 48, 5 from
81; `y` is 0, 1 from 25, 2 from 55, 4 from 95; `rst` is 1 from 40 to 42 and
`rst2` from 47 to 48. `cnt1` and `cnt2` count the `c1` and `c2` ticks with
nonblocking updates. Sampled x at the `c1` ticks 10, 30, 50, 70, 90: 1 2 4 4
5; at the `c2` ticks 20, 30, 45, 60, 90, 100: x 1 2 3 4 5 5 and y 0 1 1 2 2 4.

Clauses: a local variable assigned in an attempt belongs to that attempt
(16.10 L23833, "The local variable u is unassigned at the beginning of the
evaluation attempt"); with several semantic leading clocks (16.10
L26329-26333) "a separate copy of the local variable shall be created for
each semantic leading clock"; disable iff (16.12, L24454-24455): "If prior
to the completion of that evaluation the disable condition becomes true,
then the overall evaluation of the property results in disabled", and (L24470-24471)
"The values of variables used in the disable condition are those in the
current simulation cycle, i.e., not sampled"; actions (16.15.1 L26464):
"The pass and fail statements of an assert statement are executed in the
Reactive region", after the NBA region of the same time step (4.4).

- `l1` `keep_x = (1'b1, v = x) ##1 @(posedge c2) 1'b1 ##1 y == v` from every
  `c1` tick, so attempts overlap (30's lasts to 60, 50's to 90, 70's to
  100): 10 (v=1)→20→y30=1 P30; 30 (v=2)→45→y60=2 P60; 50 (v=4)→60→y90=2 F90;
  70 (v=4)→90→y100=4 P100; 90 open. The action prints `cnt1 cnt2`, which
  include the NBA updates of the action's own time step: 30 → 2 2, 60 → 3 4,
  90 → 5 5, 100 → 5 6.
- `l2` `(a, v = x) |=> @(posedge c2) y == v - 1`: 10 (v=1)→y20=0 P20; 30
  (v=2)→y45=1 P45; 50 vacuous P50; 70 (v=4)→y90=2 F90; 90 (v=5)→y100=4 P100.
- `l3` `e |=> two_copies`, the 16.10 L26363-26369 equivalent form of one copy
  per leading clock: `(@(posedge c1) (1'b1, v = x) ##0 v == 4) and
  (@(posedge c2) (1'b1, v = x) ##0 v == 3)`. Only 30 has e: the c1 copy is
  x(50)=4 and the c2 copy x(45)=3, so both hold: P50. The other attempts are
  vacuous: P10 P50 P70 P90.
- `dl1` `disable iff (rst) a |=> @(posedge c2) b ##1 @(posedge c1) d`: 10→b20
  →d30=1 P30; 30 waits for the `c2` tick 45 and is disabled at 40 (no
  action); 50 vacuous P50; 70→b90→ no later `c1` tick (open); 90→b100=0 F100.
- `dl2` `@(posedge c2) disable iff (rst2) b |=> @(posedge c1) d`: 20→d30=1
  P30; 30 vacuous P30; 45 waits for the `c1` tick 50 and is disabled at 47;
  60→d70=1 P70; 90 open; 100 vacuous P100.
- `s1` `a |=> strong(@(posedge c2) d ##1 d)`: 10→d20, d30 P30; 30→d45=0 F45;
  50 vacuous P50; 70→d90, d100 P100; 90→d100, then no `c2` tick: the strong
  obligation is pending at `$finish` and, being no `s_eventually`, reports
  nothing (S38-D1, A13-D7). `s2`, the same with `weak`, also leaves 90 open:
  both give P30 F45 P50 P100.

## `oracle.sv` (A01)

Twelve `unit` instances each draw a trace from a xorshift32 generator seeded
by their `SEED` parameter (16 steps of 10 time units: `a` and `b` change at
offset 4, `posedge c1` and `posedge c2` occur at offset 5 when bits 2 and 3 of
the state are set). The clocks are irregular: they tick together, apart, or
not at all in a step. Every unit asserts the same 19 properties under
`@(posedge c1)`: multiclocked sequences with `##1` and `##0`, both
implications, a multiclocked antecedent, `and`, `or`, `not`, `if`-`else`,
`nexttime`, `until` and `always` with operands on the other clock, recursion
(`ra` on c2, the weak-until `wu`, `oc2` alternating clocks, the mutual
`ph1`/`ph2`, the two-site `ts`), a strong multiclocked consequent, and
`sync_accept_on`.

The expected lines are not stored. The suite recomputes the traces and
derives every report with the test-side interpreter
[`oracle.rs`](../../../../sim_feature_completion/adv_013/oracle.rs), written
from Annex F: the derived forms of F.3.4.3, the clock rewrite rules of F.5.1
(L70999-71045), the neutral satisfaction of F.5.3.1 (L71168-71184) and the
k-fold approximations of F.7 (L71666-71689), except that an implication's
consequent is rewritten under the antecedent's end clock (A13-D1) and an
`if`-`else` is `(b |-> P1) and (!b |-> P2)`, so both branches start at the
condition's tick (A13-D3). A letter is a time step in which
a clock ticks. An attempt is reported on the first letter after which every
continuation (one more letter, then any letter repeated forever) gives the
same verdict; an attempt still undecided at `$finish` reports nothing, since
llg reports only a pending `s_eventually` obligation at the end (S38-D1,
A13-D7) and the tested properties contain none.

## Local variables in recursive and multiclocked properties (A2)

`rec_local_formal.sv`, `rec_reentry_locals.sv`, `rec_fibonacci.sv`,
`rec_check_write.sv`, `rec_local_forms.sv`, `local_two_clocks.sv`,
`local_flow.sv`, `local_if.sv` and `rec_budget.sv`. Each recursion level and
each attempt has its own locals; a local's initialization is a one-tick
match item at the semantic leading clock of the property it starts (F.4.2
`push`, decision A13-D8). Clocks rise at 5, 15, 25, ... (period 10) unless
said otherwise; a value set between ticks is the sampled value at the next
tick. Every attempt whose antecedent does not match prints a vacuous `P`
at its own tick (S38-D11), which the lists below leave out; the nonvacuous
lines were also confirmed against the industry-standard behaviour, except
the `case` property `p2` of `local_if.sv`, which that implementation does not
accept.

Clauses:

- 16.10 L23800-23802: "At the beginning of each evaluation attempt of an
  instance of a named sequence or property, a new copy of each of its local
  variables shall be created and, if present, the corresponding
  initialization assignment shall be performed."
- 16.8.2 L22290-22293: "A local variable formal argument with direction
  input or inout shall be treated like a local variable declared in an
  assertion_variable_declaration with a declaration assignment. The initial
  value for the local variable formal argument is provided by the associated
  actual argument for the instance."
- 16.13.17 L25526-25530 (restriction 4): "each actual argument expression e
  of the instance satisfies at least one of the following conditions: ...
  e is bound to a local variable formal argument of q."
- 16.14.7 L26329-26335: "If there are two or more distinct semantic leading
  clocks for an instance of a named property, then a separate copy of the
  local variable shall be created for each semantic leading clock. For each
  copy of the local variable, the initialization assignment shall be
  performed at the earliest tick of the corresponding semantic leading clock
  that is at or after the beginning of the evaluation attempt".

Expected lines:

- `rec_local_formal.sv`, `a |-> count(2)` with `count(n) = if (n > 0) (d
  and (1'b1 |=> count(n - 1)))`: d must hold for n ticks. Attempt 15 (a, d):
  d15, d25, then `count(0)` at 35 holds vacuously: P35. Attempt 45: d45, d55
  = 0: F55.
- `rec_reentry_locals.sv`, `s |-> track(1)` where every level samples `v =
  cnt` and checks `cnt == v + 1` one tick later; `cnt` counts at the falling
  edges from 20 except 50: attempt 15 holds at 25, 35 and 45 and fails at 55
  (cnt sampled 3 at both 45 and 55): F55.
- `rec_fibonacci.sv`, the 16.13.17 `fibonacci1` (L25533-25541). f1 `start
  |-> fibonacci1(a0, b0, n0, fib)`: attempt 15 (0, 1, 6) checks fib = 0, 1,
  1, 2, 3, 5 at 15..65 and ends with n = 0 at 75: P75; attempt 35 (1, 2, 3)
  checks 1, 2, 3 at 35..55: P65. f2 reads `bad`, which is 4 instead of 3 at
  55: both attempts F55. f3 `start |=> fibonacci1(a0, b0, 2, fib)` samples
  the actuals where the instance starts, one tick after the attempt:
  attempt 15 takes (1, 1) at 25 and checks fib = 1, 1 at 25 and 35: P45;
  attempt 35 takes (2, 3) at 45 and checks 2, 3: P65.
- `rec_check_write.sv`, the 16.13.17 `check_write` /
  `check_write_data_beat` (L25637-25686), `expected_data[i*8+:8]` being byte
  i from the most significant end. Request tag 3 at 15 (A1, A2): beat A1 at
  35, last beat A2 at 55, no retry at 65: P65. Request tag 5 at 75: beat B1
  at 95, a wrong second beat at 105: F105. Request tag 7 at 125: beat C1 at
  145, retry at 155 restarts at beat 0, beats C1 at 175 and C2 (last) at 185,
  no retry at 195: P195.
- `rec_local_forms.sv` (`c2` rises at 17, 37, 57, ...; `s` at 15 and 55).
  m1, mutual recursion `up(k)`/`down(k)` with `k + 1` per tick, d then e:
  attempt 15 from 2 holds to 55 and fails at 65 (e = 8, not 7); attempt 55
  from 6 fails at 65 too: F65 F65. m2, `sw(p, q)` swaps its locals by value:
  attempt 15 (2, 9) needs d = 9 at 25: F25; attempt 55 (6, 7) needs d = 7 at
  65: F65. m3, `s |=> cs(d)` with `logic [3:0] m = k + 1`: at 25 k = 3 and
  d + 1 == m: P25; at 65 k = 9 and e (8) is not m (10): F65. m4, `(s, v = e)
  |=> keep(3, v)` reads the antecedent's v at every level: attempt 15 (v = 9)
  P55, attempt 55 (v = 7) P95. m5, `s |=> tc(x)` with one copy of v per
  clock: attempt 15 takes x = 3 at c2 17 and x = 4 at 25: P25; attempt 55
  takes x = 2 at c2 57: F57. m6, `not ((@clk v == 1) or (@c2 v == 2))`:
  P25, and F57 (x = 2 at 57). m7, recursion under `nexttime` never fails:
  no nonvacuous line.
- `local_two_clocks.sv` (`c1` rises at 10, 30, ...; `c2` at 16, 36, ...),
  the former `neg_local_two_clocks.sv`: attempt 10 gives the c2 copy x = 3
  at 16 and the c1 copy x = 4 at 30: P30; attempt 50 gives the c2 copy x = 4
  at 56: F56.
- `local_flow.sv`, `(a, x = d) |=> ((b, y = e) |=> (x == y + 1))`: x flows
  through the inner antecedent, which assigns only y: attempt 15 (x = 5,
  y = 4 at 25): P35; attempt 45 (x = 7, y = 2): F65.
- `local_if.sv`, conditions on the antecedent's x: attempt 15 (x = 5) needs
  !b at 35 (b = 1): p1 F35, p2 F35; attempt 45 (x = 6) needs b at 65 in p1
  (b = 0): F65, and !b in p2: P65.
- `rec_budget.sv`: `grow(n)` starts two recursive instances per tick from
  15; with `LLG_SEQUENCE_THREAD_LIMIT=200` the live instances pass the budget
  at 75 and the run stops with the budget report, before `done`.

## Rejections

| Fixture | Diagnostic | Why |
| --- | --- | --- |
| `neg_mc_repetition.sv` | `sequence operator repetition combines differently clocked or multiclocked sequence operands; only ##1 and ##0 may (IEEE 1800-2009 16.14.1)` | 16.14.1 L25924: "Differently clocked or multiclocked sequence operands cannot be combined with any sequence operators other than ##1 and ##0." |
| `neg_mc_first_match.sv` | `sequence operator first_match combines ...` | Same clause. |
| `neg_mc_delay.sv` | `multiclocked sequence operands cannot be combined with any sequence operators other than ##1 and ##0` (frontend) | Same clause; L25926-25927 give `@(posedge clk1) s1 ##2 @(posedge clk2) s2` as illegal. |
| `neg_mc_empty.sv` | `sequence admits an empty match; this is not allowed when concatenating differently clocked sequences` (frontend) | 16.14.1 L25903-25904: "the maximal singly clocked subsequences are required to admit only nonempty matches". |
| `neg_leading_clocks.sv` | `assertion expression does not have a unique leading clock` (frontend) | 16.17 e) L27666-27668: the maximal property "shall have a unique semantic leading clock". |
| `neg_inferred_no_clock.sv` | `no inferred clock in this context` (frontend) | 16.15.7 L27196-27197: with no current resolved event expression "an error shall be issued". |
| `neg_global_clock_missing.sv` | `no global clocking is available in this scope` (frontend) | 14.14 L19974-19975: "It shall be an error to invoke the $global_clock system function if there is no global clocking declaration". |
| `neg_rec_not.sv` | `recursive property instance cannot have the operators 'not', ...` (frontend) | 16.13.17 L25473-25475 (restriction 1): "The negation operator not and strong operators ... cannot be applied to any property expression that instantiates a recursive property." |
| `neg_rec_disable.sv` | `recursive properties cannot have disable iff conditions` (frontend) | 16.13.17 L25499-25500 (restriction 2): "The operator disable iff cannot be used in the declaration of a recursive property." |
| `neg_rec_no_advance.sv` | `recursive property instance must occur after a positive advance in time` (frontend) | 16.13.17 L25515-25516 (restriction 3): "every instance of p must occur after a positive advance in time." |
| `neg_rec_args.sv` | `cannot refer to argument 'x' in an argument to a recursive property ...` (frontend) | 16.13.17 L25526-25530 (restriction 4). |
| `neg_rec_reentry_args.sv` | `recursive property instance does not repeat an enclosing expansion with the same actual arguments, which llg does not support (SV 16.13.17)` (llg limit) | Legal by L25526-25528 (restriction 4: "e is itself a formal argument of p"); the actuals rotate three formals, so the frontend's two expansions are not repeated by the re-entry, which llg would have to call. |
| `neg_rec_locals_args.sv` | `recursive property instance re-enters a property with local variables after its other actual arguments changed; this is not supported (SV 16.13.17)` (llg limit) | Legal by L25526-25530; the non-local actual changes from `e` to `d` after the first level, and the frontend leaves the third level unexpanded, so llg has no actuals to bind the local formal from. |
| `neg_rec_locals_clocks.sv` | `recursive property instance re-enters a property whose local variables are initialized at more than one semantic leading clock; this is not supported (SV 16.13.17, 16.14.7)` (llg limit) | Legal; 16.14.7 L26331-26335 gives each semantic leading clock its own copy, and a recursion level would need one per copy. |
| `neg_local_clocks_implies.sv` | `the local variables of the property instance would be initialized at more than one semantic leading clock below an operator other than `and`, `or` or `not`; this is not supported (SV 16.14.7)` (llg limit) | Legal by 16.14.7 L26331-26335; F.4.2 (L70953, L70964-70965) defines the split only for `and`, `or` and `not`, which llg implements. |

## Other implementations

The expected lines follow the clause text. Other simulators were seen to
differ in three ways, which the fixtures keep:

- Some simulators skip the pass action of a vacuous success (an implication
  whose antecedent has no match, an `if` without `else` whose condition is
  false). llg runs it (decision S38-D11), because 20.12 L35137-35139 says "By
  default, the pass action is executed on vacuous success". This gives the
  extra `P` lines of `clocks.sv`, `domains.sv`, `flow.sv`, `oracle.sv` and
  `recursion.sv`.
- Some simulators treat the declaration initializers of `c1` and `c2` as a
  change at time 0, which would add `g3 0`, `k1 F 0` and `k2 F 0` to
  `clocks.sv` and end the four `g3` ticks at 30. llg applies them before any
  procedure starts (decision T0-D1; 6.8 L5286-5288).
- Some simulators keep clocking a recursive instance by the clock that
  started the attempt even when its recursive call has its own clocking event
  (`r10` in `recursion.sv` and `p14` in `oracle.sv`:
  `x and (1'b1 |=> @(posedge c2) on_c2(x))`), so they report `r10` F50 for
  the attempt at 10 instead of F45. llg checks the recursive call at the
  `posedge c2` ticks, because 16.14.3 L26027-26030 says the scope of a
  clocking event flows "until it is replaced by a new clocking event".
