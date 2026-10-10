# SIM-014 repeated waits, intra-assignment timing and event triggers

V2001 §§9.6, 9.7.7 and IEEE 1800-2009 §§4.9.3, 4.9.4, 6.12.2, 6.21,
9.4.2.1, 9.4.5, 10.4.1, 10.4.2, 12.7.2, 13.3.2, 13.4 and 15.5 supply the
oracles. Every positive source runs through the public CLI in both optimizer
modes on the legacy, compact/portable and compact/GMP value backends
(`run_case_backend_parity`); `destinations` and `nb_triggers` also run after
snapshot/Db destruction at native O0/O3. All `.out` files are hand-derived
from the clauses and the timelines below. A repeating event `e` (or `tick`)
occurs at fixed times, and every timed statement is issued at a time when
that event does not occur, so no result depends on the order of processes
within a time step. Line numbers refer to `SystemVerilog-1800-2009.txt` (SV)
and `Verilog-1364-2001.txt` (V2001), the `pdftotext -layout` extractions.

## Positives

| Fixture | Clauses | Derivation |
| --- | --- | --- |
| `repeat_counts` | SV 12.7.2, 9.4.5, 6.12.2 | `e` at 2, 4, …. From 1: `repeat (3)` ends at 6. At 7: counts 0 and -2 (`int`) and -1 (`logic signed [3:0]`) run zero times; `logic [1:0]` 2'b11 is the unsigned 3 and ends at 12. At 13: an all-X count and a count with one Z bit run zero times (§12.7.2); real 1.4 rounds to 1 and ends at 14. 1.5 rounds away from zero to 2 (§6.12.2): 15 to 18. -0.4 rounds to 0 at 19. `shortreal` 2.0: 19 to 22. Literal 0.5 rounds to 1: 23 to 24. From 25, `repeat (n) @e` with `n = 2` ends at 28 although `n` becomes 10 at 27: the count is evaluated once when the loop starts. |
| `intra_counts` | SV 9.4.5, 6.12.2, 10.4.2 | Blocking: from 1, `a = repeat (n) @e b` captures `b = 11` and `n = 2`; the change of both at 3 is ignored; `a = 11` at 4. At 5 counts 0, -1, X, a Z bit and real 0.4 (rounds to 0) assign immediately; real 1.5 (2 waits) assigns at 8. Nonblocking (issued by the second process, operands changed by the third after each issue): from 11, `c <= repeat (m) @e d` with `m = 2`, `d = 66` updates `c` at 14 (`xx` at 13, `66` at 15); counts 0 (15), -5 (17) and Z (18) update in the NBA region of the issuing step, so the same-step display still shows the old value; real 2.0 from 19 updates at 22. |
| `destinations` (A01) | SV 4.9.3, 4.9.4, 9.4.5, 10.4.1, 10.4.2 | `e` at 4, 8, …; blocking assignments are issued at 1 mod 4 and their selectors and RHS operands change one unit later. Update-time destinations: `m[i]` (i 0→2) writes `m[2] = 11` (the captured RHS, not 22); `s.f[i]` (0→5); `v[i+:2]` after two events (0→6); queue `q[i]` (0→3); dynamic `dyn[i]` (0→1); associative `aa[key]` ("a"→"b", so key "a" is never created); `h.x` with `h` rebound from `h1` to `h2` writes `h2.x`; `r = rr * 2.0` captures 3.0; `st = repeat (2) @e sv` captures "ab". Issue-time destinations (from 53, operands changed by the third process): `n[j]` with `j = 1` (changed to 3 at 54) writes `n[1] = 33` (RHS changed to 44 at 54); `t.g[j]` (`j = 2`, changed at 58) after two events; `w[j+:2]` (`j = 1`); `nr <= @e nrv * 2.0` captures 5.0; `ns <= repeat (2) @e {nsv, "!"}` captures "one!" (`nsv` becomes "two" at 74), updated at 80. |
| `container_elements` (A01) | SV 4.9.3, 9.4.5, 10.4.1 | `e` at 4, 8, …. `names[i] = @e text` (fixed string array) issued at 1 with `i = 0`, `text = "hi"`; both change at 2, so `names[1] = "hi"` at 4. `sq[i] = repeat (2) @e "q"` (string queue) from 5, `i` changed to 2 at 6: `sq[2]` at 12. `ar[key] = #2 1.25` (real associative array) from 13, key changed to "b" at 14: `ar["b"]` at 15 and key "a" is never created. `boxes[i] = @e b1` (dynamic array of class handles) from 17 captures the object with `id` 7; `i = 1` and `b1 = null` at 18: `boxes[1]` gets the object at 20 and `boxes[0]` stays null. |
| `nb_triggers` (A02) | SV 15.5.1, 10.4.2, 4.9.4, 9.4.5, 15.5.5.2 | `tick` at 2, 4, …; each `always @x/@y/@z/@(evs[k])` prints when its event occurs. At 1 `->> repeat (2) @tick src` with `src` naming `x` does not block (the issuer prints at 1 and `x.triggered` is 0); `src` is rebound to `y` at 1 and nulled at 3, yet `x` occurs at 4 after exactly two ticks. `->> #3 src` at 7 (`src` = `z`, rebound to `x` afterwards) triggers `z` at 10. `->> @tick src` at 11 (`y`, then null) triggers `y` at 12. Triggers through a null handle at 13 have no effect (§15.5.5.2). At 15 `->> repeat (n) @tick evs[i]` with `i = 1`, `n = 2` (changed to 2 and 5 afterwards) triggers `evs[1]` at 18. Counts 0 (19), -3 (21) and X (23) trigger in the issuing step; real 1.5 rounds to 2: 25 to 28. An automatic task issues `->> repeat (count) @tick ev` on its event formal at 31 and returns; `z` occurs at 34. |
| `event_formals` | SV 9.4.2.1, 13.5, 15.5.3, 15.5.5, 15.5.5.3 | `@(posedge clk or ev or sig)` wakes at 1 (posedge), 3 (`->ev`) and 4 (`sig`), not at 2 (negedge): 3 hits. Event formals share the actual's object: `waiter(ev3)` wakes at 7; recursive `rec(ev2, 3)` wakes at 8, 9 and 11 (the last trigger comes from `fire_later(ev2, 1)`); the function `same` compares formals (1 for `ev, ev`, 0 for `ev, ev2`) and `live(ev2)` is 1 because `ev2` triggered at 11. Class methods: `bh.wait_on(a)` dispatches to `Derived` (+10) at 12; `chain(b, 2)` recurses through `@(e1 or posedge clk2)`: `->b` at 13 and the posedge at 14 add 100 each (210); `later(c)` issues `->> #1 c`, which occurs at 15; `copy` binds the output event formal to `a`, so `same(got, a)` is 1, `same(a, b)` 0 and `same(got, null)` 0. |
| `event_values` | SV 15.5.3, 15.5.4, 15.5.5, 15.5.5.1, 15.5.5.2, 15.5.5.3 | `->a` at 0: `a.triggered` is 1 at 0 and after `#0`, and 0 at 1. `fork ->b; wait (b.triggered); join` completes at 1 whichever branch runs first. A null event's `.triggered` is 0 and `->n1` does nothing. `al = a` makes `al` share `a`'s object: `==`, `===` are 1, `!=`, `!==` and `== null` are 0, and `a == b` is 0; `if (n1)` is false, `if (!al)` false, `n1 ? 1 : 0` is 0 and `al ? 1 : 0` 1. `@al` wakes when `->a` runs at 2. A process already blocked on `e1` never wakes after `e1 = e2` (§15.5.5.1), while `->e1` now triggers `e2`'s object. `wait_order (a, b, c)`: a 5, b 6, a again 7 (an earlier event may retrigger), c 8: success at 8. a 9, c 10: failure at 10 (`ok = 0`). `wait_order (a, b) else …` fails when `b` comes first (12). After `->a` at 12, `wait_order (a, b)` succeeds at 13 when `b` occurs: the first event may use its triggered state. |
| `wait_order_error` | SV 15.5.4 | `wait_order (a, b);` without a fail statement fails at 1 when `b` triggers first: llg reports `severity error` at `tb:9:9` on stderr and the process continues with the next statement. |
| `cancellation` | SV 9.7, 9.6.3, 9.4.5 | A branch blocked in `repeat (3) @e` is killed at 3 after one event; it never prints. A new `repeat (3) @e` from 5 counts 6, 8, 10. A branch in `x = repeat (3) @e 9` is killed at 15 after two events: `x` stays 0. `disable fork` at 23 ends a standalone and a blocking repeated wait after one event each; `y` stays 0, and `repeat (2) @e` from 25 ends at 28. |
| `task_timing` | SV 9.4.5, 13.3, 13.5 | `hold(2, 5)` (automatic) assigns `loc = repeat (2) @e v` at 4, then `text = @e $sformatf(...)` at 6 (`e` already occurred at 4 when the control is reached). Recursive `digits(3)` takes one event per level: 8, 10, 12, so `h = 321`. The static task `queue_writes` at 13 issues `arr[1] <= repeat (2) @e 8'h3c` (updated at 16) and `label <= @e "late"` (updated at 14) and returns at once. |
| `v2001_forms` (`--edition v2001`) | V2001 9.7.7, 9.6 | An `integer` count of -3 waits zero times; `reg [1:0]` 2'b11 waits three events (2, 4, 6). `c <= repeat (2) @e 8'h77` issued at 3 updates at 6. `a = repeat (n) @e b` from 7 with `n = 2` assigns 12 at 10. `m[i] = @e 8'h5a` from 11 with `i` changed to 2 at 12 writes `m[2]`. |

Adopted FND-002 witnesses (source unchanged): `repeat_real_witness.v`
(L-F06-05-03; an integral-valued real count, 2.0), `persistent_repeat_nba_witness`
(L-F06-04-04; `s <= repeat (2) @(posedge clk) "ok"`), `repeat_nested_timing_witness`
(L-F06-04-01, L-F06-05-02; a repeat loop around `x = repeat (2) @(posedge clk) 7`)
and `trigger_repeat_witness` (`->> repeat (2) @(posedge clk) e`).

## Negatives

Language rules, rejected in both optimizer modes:

- `neg_class_property_nba`: `h.x <= @e 5`. SV 6.21 forbids nonblocking
  writes to class properties; llg reports the property and the source location.
- `neg_automatic_event_nba` and the FND-002 `neg_automatic_nba_witness`: an
  automatic task variable as an NBA target, with a repeated event control or a
  real value (SV 6.21, 13.3.2), reported by the frontend.
- `neg_automatic_in_nba_control`: an automatic variable in the event control
  of an NBA (SV 13.3.2), reported by the frontend.
- `neg_dynamic_element_nba`: a queue element as an NBA target (SV 6.21),
  reported by the frontend.
- `neg_function_event_control`: an intra-assignment event control in a
  function (SV 13.4 a), reported by the frontend.

## Clause text

V2001 §9.7.7 (L9896-9900):

> The intra-assignment delay and event control can be applied to both blocking assignments and nonblocking
> assignments. The event expression shall be resolved to a 1-bit value. The repeat event control shall specify
> an intra-assignment delay of a specified number of occurrences of an event. If the repeat count literal, or
> signed reg holding the repeat count, is less than or equal to 0 at the time of evaluation, the assignment occurs
> as if there is no repeat construct.

SV §9.4.5 (L12123-12124, L12138-12149):

> An intra-assignment delay or event control shall delay the assignment of the new value to the left-hand side,
> but the right-hand expression shall be evaluated before the delay, instead of after the delay.

> The intra-assignment delay and event control can be applied to both blocking assignments and nonblocking
> assignments. The repeat event control shall specify an intra-assignment delay of a specified number of
> occurrences of an event. If the repeat count literal, or signed variable holding the repeat count, is less than or
> equal to 0 at the time of evaluation, the assignment occurs as if there is no repeat construct.
>
> repeat (-3) @ (event_expression)
> // will not execute event_expression.

SV §12.7.2 (L18159-18160):

> The repeat-loop executes a statement a fixed number of times. If the expression evaluates to unknown or
> high impedance, it shall be treated as zero, and no statement shall be executed.

SV §6.12.2 (L5531-5533):

> Real numbers shall be converted to integers by rounding the real number to the nearest integer, rather than
> by truncating it. Implicit conversion shall take place when a real number is assigned to an integer. If the frac-
> tional part of the real number is exactly 0.5, it shall be rounded away from zero.

SV §4.9.3 (L3529-3531) and §4.9.4 (L3536-3539):

> When the process is returned (or if it returns immediately if no delay is specified), the process performs the
> assignment to the left-hand side and enables any events based upon the update of the left-hand side. The val-
> ues at the time the process resumes are used to determine the target(s).

> A nonblocking assignment statement (see 10.4.2) always computes the updated value and schedules the
> update as an NBA update event, either in the current time step if the delay is zero or as a future event if the
> delay is nonzero. The values in effect when the update is placed in the event region are used to compute both
> the right-hand value and the left-hand target.

SV §10.4.1 (L13024-13025) and §10.4.2 (L13070-13071):

> If variable_lvalue
> requires an evaluation, it shall be evaluated at the time specified by the intra-assignment timing control.

> If variable_lvalue requires an evaluation, it shall be evaluated at the same time as the
> expression on the right-hand side.

SV §6.21 (L7007-7008):

> Automatic variables and members or elements of dynamic variables—class properties and dynamically
> sized variables—shall not be written with nonblocking, continuous, or procedural continuous assignments.

SV §13.3.2 (L18648-18651):

> —      They shall not be assigned values using nonblocking assignments or procedural continuous
> assignments.
> —      They shall not be referenced by procedural continuous assignments or procedural force statements.
> —      They shall not be referenced in intra-assignment event controls of nonblocking assignments.

SV §13.4 (L18673-18674):

> a)     A function shall not contain any time-controlled statements. That is, any statements containing #,
> ##, @, fork, wait, wait_order, or expect.

SV §9.4.2.1 (L11870-11872):

> The logical or of any number of events can be expressed so that the occurrence of any one of the events trig-
> gers the execution of the procedural statement that follows it. The keyword or or a comma character (,) is
> used as an event logical or operator.

SV §15.5.1 (L20708-20711):

> Nonblocking events are triggered using the ->> operator. The effect of the ->> operator is that the statement
> executes without blocking and it creates a nonblocking assign update event in the time in which the delay
> control expires or the event control occurs. The effect of this update event shall be to trigger the referenced
> event in the nonblocking assignment region of the simulation cycle.

SV §15.5.3 (L20727-20728, L20735-20737):

> SystemVerilog can distinguish the event trigger itself, which is instantaneous, from the event’s triggered
> state, which persists throughout the time step (i.e., until simulation time advances).

> The triggered event property evaluates to true if the given event has been triggered in the current time
> step and false otherwise. If event_identifier is null, then the triggered event property evaluates to
> false.

SV §15.5.4 (L20810-20811, L20822-20830):

> For wait_order to succeed, at any point in the sequence, the subsequent events, which shall all be untrig-
> gered at this point or the sequence would have already failed, shall be triggered in the prescribed order.

> Preceding events are not limited to occur only once. In other words, once an event occurs in the prescribed
> order, it can be triggered again without causing the construct to fail.
>
> Only the first event in the list can wait for the persistent triggered property.
>
> The action taken when the construct fails depends on whether the optional action_block else statement (the
> fail statement) is specified. If it is specified, then the given statement is executed upon failure of the con-
> struct. If the fail statement is not specified, a failure generates a run-time error.

SV §15.5.5 (L20856-20857), §15.5.5.1 (L20888-20890), §15.5.5.2 (L20912) and
§15.5.5.3 (L20930):

> When one event is assigned to another, the synchronization queue of the source event is shared by
> both the source and the destination event.

> When events are merged, the assignment only affects the execution of subsequent event control or wait
> operations. If a process is blocked waiting for event1 when another event is assigned to event1, the cur-
> rently waiting process shall never unblock.

> Triggering a null event shall have no effect.

> —      Test for a Boolean value that shall be 0 if the event is null and 1 otherwise

SV §9.7 (L12631-12632) and §9.6.3 (L12548):

> The kill() function terminates the given process and all its subprocesses, that is, processes spawned using
> fork statements by the process being killed.

> The disable fork statement terminates all active descendants (subprocesses) of the calling process.

## llg decisions

Portable cases in `tests/fixtures/sim/lrm_decisions/` (see
`docs/lrm_decisions.md`): `S14-D1_real_repeat_count` (real counts round per
§6.12.2), `S14-D2_nonpositive_repeat_immediate` (a nonpositive, X or Z count
assigns or triggers without waiting), `S14-D3_nb_trigger_target_at_issue`
(`->>` triggers the event its operand named at issue),
`S14-D4_wait_order_first_triggered` (the first event may be satisfied by its
triggered state), `S14-D5_class_property_nba` (negative) and
`S14-D6_wait_order_error_continues` (a failing wait_order without a fail
statement reports an error and the process continues).
