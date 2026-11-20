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
| `container_element_selects` (FIN-001 G3) | SV 9.4.5, 10.4.1, 7.8.6 | `e` at 4, 8, …. Selects inside one packed element of a resizable container, value captured at issue, element, index or key and the element's other bits taken when the control completes. `q[i][3:0] = @e v` from 0 with `i = 0`, `v = 4'hA`; at 1 `i = 2`, `v = 4'h1` and `q[2] = 8'hF0`: at 4 `q[2]` becomes `fa` (`q[0]`, `q[1]` stay `00`). `d[i][i+4 +: 2] = repeat (2) @e 2'b11` from 5 (`i = 0`); at 6 `i = 1` and `d[1] = 8'h01`: at 12 bits 6:5 of `d[1]` are set, `61`; `d[0]` keeps the new element's `xx`. `ai[i][7] = #2 1'b1` from 13 with `i` changed from 3 to 5 at 14: `ai[5] = 80` at 15, `ai[3]` stays `00`. `as[key][15:8] = @e 8'h5A` from 16 (`key = "a"`); at 17 `key = "b"` and `as["b"] = 16'h00C3`: `5ac3` at 20, `as["a"]` stays `0000`. `as[key][3:0] = #1 4'h9` with the absent key `"c"`: at 22 the element is read as the 4-state default `'X` (§7.8.6, with its warning on stderr) and gets the low nibble: `xxx9`, three entries. |
| `nb_triggers` (A02) | SV 15.5.1, 10.4.2, 4.9.4, 9.4.5, 15.5.5.2 | `tick` at 2, 4, …; each `always @x/@y/@z/@(evs[k])` prints when its event occurs. At 1 `->> repeat (2) @tick src` with `src` naming `x` does not block (the issuer prints at 1 and `x.triggered` is 0); `src` is rebound to `y` at 1 and nulled at 3, yet `x` occurs at 4 after exactly two ticks. `->> #3 src` at 7 (`src` = `z`, rebound to `x` afterwards) triggers `z` at 10. `->> @tick src` at 11 (`y`, then null) triggers `y` at 12. Triggers through a null handle at 13 have no effect (§15.5.5.2). At 15 `->> repeat (n) @tick evs[i]` with `i = 1`, `n = 2` (changed to 2 and 5 afterwards) triggers `evs[1]` at 18. Counts 0 (19), -3 (21) and X (23) trigger in the issuing step; real 1.5 rounds to 2: 25 to 28. An automatic task issues `->> repeat (count) @tick ev` on its event formal at 31 and returns; `z` occurs at 34. |
| `event_formals` | SV 9.4.2.1, 13.5, 15.5.3, 15.5.5, 15.5.5.3 | `@(posedge clk or ev or sig)` wakes at 1 (posedge), 3 (`->ev`) and 4 (`sig`), not at 2 (negedge): 3 hits. Event formals share the actual's object: `waiter(ev3)` wakes at 7; recursive `rec(ev2, 3)` wakes at 8, 9 and 11 (the last trigger comes from `fire_later(ev2, 1)`); the function `same` compares formals (1 for `ev, ev`, 0 for `ev, ev2`) and `live(ev2)` is 1 because `ev2` triggered at 11. Class methods: `bh.wait_on(a)` dispatches to `Derived` (+10) at 12; `chain(b, 2)` recurses through `@(e1 or posedge clk2)`: `->b` at 13 and the posedge at 14 add 100 each (210); `later(c)` issues `->> #1 c`, which occurs at 15; `copy` assigns `a` to its output event formal, which is not copied out (decision S13-D3): `same(got, a)` is 0, `same(a, b)` 0 and `same(got, null)` 0. |
| `event_formals_value_calls` (FIN-001 G3) | SV 13.3, 13.5, 13.5.2, 15.5.5.1 | §13.3 L18476-18477: "output // copy value out at end", "inout // copy in at beginning and out at end"; §13.5.2 L19138-19139 for `ref`. Output, inout and `ref` event formals of functions called in an expression, with integral, real, string, handle, chandle, record and queue results, and input event formals of string and handle functions, bind event variables and class event properties, also through a class method and a virtual method. A class property actual is copied in at the call and stored back at the return into the object the actual then names: `fo` reads the old value, and `fmove` writes the object `c` names after its body rebinds `c`, so `c2.ev` becomes `g`. An output event formal is not copied out (decision S13-D3): its actual keeps its object, so the lines after an output formal show 0 where an inout shows 1. The output agrees with industry-standard behaviour. |
| `event_function_results` | SV 13.4.1, 6.17, 15.5.5, 15.5.5.1, 15.5.5.3 | §13.4.1 L18796-18797: "The function definition shall implicitly declare a variable, internal to the function, with the same name as the function. This variable has the same type as the function return value." §6.17 L5963: "When assigned another event variable, both event variables refer to the same synchronization object"; L5966-5967: an event variable without an initial value "is initialized to a new synchronization object" (decision S13-D8: a new one at each call). `j = pass(k)`, `m = fstat(d)` (result assigned by name in a static function), `p = pass(pass(a))`, `x = c.get()` and `y = C::sget(x)` share the source's object: 1 each; `n = fnull()` is null; `e1 = fdef()` and `e2 = fdef()` (no result set) are two new events, so `e1==e2` is 0 and `->e1` at 4 wakes nothing. Comparisons of calls: `pass(k)==k` 1, `pass(k)==null` 0, `fnull()==null` 1, `fdef()==null` 0, `pick(0, a, b)==a` 1, `pick(1, a, b)==a` 0; `if (pass(a))` is true (§15.5.5.3); `c.ev = pass(s)` makes `c.ev==s` 1. Waits from 1: `->k` at 2 wakes `@(j)`; `->d` at 3 `@(m)`; `->a` at 5 `@(pass(a))`; `->b` at 6 and `->r` at 7 `@(pick(1,a,b) or r)`; `->s` at 8 `@(c.get())`, since `c.ev` names `s`'s object; `t_wait(pass(q))` from 9 wakes on `->q` at 10; `trig(r)` triggers its result, `r`'s object, at 11. Each event wakes one waiter. The output agrees with industry-standard behaviour. |
| `class_event_formals` (FIN-001 G3) | SV 13.5, 13.5.2, 15.5.5 | §13.5.2 L19185-19187: "an inout argument is copied twice: once from the actual into the argument when the subroutine is called and once from the argument into the actual when the subroutine returns"; §13.3 L18476 "output // copy value out at end", which decision S13-D3 departs from for events. `pick(c.ev, 0)` returns at 1 with `o = g`, which is not copied out: `c.ev` keeps its object (`out g 0 h 0`), so `@(c.ev)` does not wake on `->g` at 2 and wakes on `-> c.ev` at 3. With `c.ev = g`, `swap(c.ev)` starts with `g`'s object, wakes on `->g` at 4 and sets `x = h`; the copy-back makes `c.ev` name `h` (`inout g 0 h 1`). The method `c.relay(d.ev)` assigns `c.ev` to its output at 5, which leaves `d.ev` null (`d.ev==c.ev 0 d.ev==null 1`). The output agrees with industry-standard behaviour. |
| `string_element_events` | SV 9.4.2 | §9.4.2 lists a change in the value of an expression as an event: `@` on elements of fixed (also 2-D), queue, dynamic and associative string arrays and on a run-time-indexed element wakes on a change of that element's value, not on an equal store or a store to another element; a resized queue changes `q[0]` (8); after `i` moves, the selected element is the one watched (`fi`). The output agrees with industry-standard behaviour. |
| `class_event_ref_held` (FIN-001 G3) | SV 13.5.2, 15.5.5.1, 8.27 | A task holds a `ref` to a class event or string property while the last handle to its object is dropped and collections run at every safe point: the callee's frame keeps the object, so its rebinding, trigger (at 3, waking `@(g)`) and string store (at 5) stay valid. These lines agree with industry-standard behaviour. A level wait on the property's identity (`wait (d.ev == g)`) wakes at the rebinding through the formal, as after `d.ev = g` (§15.5.5.1, §13.5.2 L19138-19141: the formal is the property): 7 for `rebind`, 8 for `late`, which returns only at 11, 14 through the forwarding task `forward`, 18 through a virtual method. A rebinding inside a fork-join branch wakes at the rebinding (21, before `in_branch` returns at 22). An output event formal the `ref` formal is forwarded to is not copied out (decision S13-D3), so `o2` and `o3` never name `g`: after `out_set` returns at 25 and 31 and `via_output` and `via_branch_output` (whose `out_set` call runs in a fork-join branch) return at 27 and 33, the checks at 28 and 34 print 0. A polling probe of industry-standard behaviour shows the same: the property never changes. Some simulators reject an event operand in a `wait` expression; that part is derived by hand from 15.5.5.1. |
| `class_event_ref_formal` (FIN-001 G3) | SV 13.5.2, 15.5.5.1 | §13.5.2 L19143-19145 lists a class property as a `ref` actual: `t(c.ev)` triggers and waits on the property's own event, `rebind(c.ev, g)` merges it with `g` at once, a task sees it across suspension, and value calls bind `ref` event formals too. Agrees with industry-standard behaviour. |
| `event_values` | SV 15.5.3, 15.5.4, 15.5.5, 15.5.5.1, 15.5.5.2, 15.5.5.3 | `->a` at 0: `a.triggered` is 1 at 0 and after `#0`, and 0 at 1. `fork ->b; wait (b.triggered); join` completes at 1 whichever branch runs first. A null event's `.triggered` is 0 and `->n1` does nothing. `al = a` makes `al` share `a`'s object: `==`, `===` are 1, `!=`, `!==` and `== null` are 0, and `a == b` is 0; `if (n1)` is false, `if (!al)` false, `n1 ? 1 : 0` is 0 and `al ? 1 : 0` 1. `@al` wakes when `->a` runs at 2. A process already blocked on `e1` never wakes after `e1 = e2` (§15.5.5.1), while `->e1` now triggers `e2`'s object. `wait_order (a, b, c)`: a 5, b 6, a again 7 (an earlier event may retrigger), c 8: success at 8. a 9, c 10: failure at 10 (`ok = 0`). `wait_order (a, b) else …` fails when `b` comes first (12). After `->a` at 12, `wait_order (a, b)` succeeds at 13 when `b` occurs: the first event may use its triggered state. |
| `wait_order_error` | SV 15.5.4 | `wait_order (a, b);` without a fail statement fails at 1 when `b` triggers first (after the line printed before the trigger): llg reports `severity fatal` at `tb:9:9` on stderr and the run ends at once with exit status 1; neither the waiting process nor the parent continues (S14-D6). |
| `cancellation` | SV 9.7, 9.6.3, 9.4.5 | A branch blocked in `repeat (3) @e` is killed at 3 after one event; it never prints. A new `repeat (3) @e` from 5 counts 6, 8, 10. A branch in `x = repeat (3) @e 9` is killed at 15 after two events: `x` stays 0. `disable fork` at 23 ends a standalone and a blocking repeated wait after one event each; `y` stays 0, and `repeat (2) @e` from 25 ends at 28. |
| `task_timing` | SV 9.4.5, 13.3, 13.5 | `hold(2, 5)` (automatic) assigns `loc = repeat (2) @e v` at 4, then `text = @e $sformatf(...)` at 6 (`e` already occurred at 4 when the control is reached). Recursive `digits(3)` takes one event per level: 8, 10, 12, so `h = 321`. The static task `queue_writes` at 13 issues `arr[1] <= repeat (2) @e 8'h3c` (updated at 16) and `label <= @e "late"` (updated at 14) and returns at once. |
| `repeat_nba_progress` | SV 9.4.5, 10.4.2, 15.5.1 | The first process issues at 0, without waiting, seven detached forms: `t <= repeat (3) @(e) "rep"` (string), `r <= repeat (3) @(e) 1.5` (real), `h <= repeat (3) @(e) k` (class handle, `k.v = 7`), `x <= repeat (3) @(posedge g.w) 5` (an edge of a class property, evaluated by the waiting process), `y <= repeat (3) @(posedge clk) 6` (module variable), `z <= repeat (0) @(e) 9` (count 0: updated in the NBA region at 0) and `->> repeat (2) @(posedge g.w) f`. The second process triggers `e` and raises `g.w` and `clk` at 1, 3 and 5 (lowered at 2 and 4) and prints at 1 (before the first events), 2, 4 and 6. At 1 and 2 nothing but `z` has changed; at 4 (two events) only `f` has occurred (its second edge at 3), so `fired` is 1; at 6 every target holds its value (`rep 1.5`, `h` not null with `v = 7`, `x = 5`, `y = 6`). |
| `v2001_forms` (`--edition v2001`) | V2001 9.7.7, 9.6 | An `integer` count of -3 waits zero times; `reg [1:0]` 2'b11 waits three events (2, 4, 6). `c <= repeat (2) @e 8'h77` issued at 3 updates at 6. `a = repeat (n) @e b` from 7 with `n = 2` assigns 12 at 10. `m[i] = @e 8'h5a` from 11 with `i` changed to 2 at 12 writes `m[2]`. |

Adopted FND-002 witnesses (source unchanged unless noted): `repeat_real_witness.v`
(L-F06-05-03; an integral-valued real count, 2.0), `persistent_repeat_nba_witness`
(L-F06-04-04; `s <= repeat (2) @(posedge clk) "ok"` with posedges at 1 and 3; FIN-001
added a read at 2, between the edges, where `s` is still empty), `repeat_nested_timing_witness`
(L-F06-04-01, L-F06-05-02; a repeat loop around `x = repeat (2) @(posedge clk) 7`)
and `trigger_repeat_witness` (`->> repeat (2) @(posedge clk) e`).

## Negatives

Language rules, rejected in both optimizer modes:

- `neg_class_property_nba`: `h.x <= @e 5`. SV 6.21 forbids nonblocking
  writes to class properties; llg reports the property and the source location.
- `neg_class_record_property_nba_whole`, `_member` and `_string`: `c.r <= v`,
  `c.r.a <= #1 6` and `c.r.s <= @e "z"` on a record property with a string
  member, run with `-D N=3` and `-D N=16` (both record storage forms). The same
  SV 6.21 rule (`SystemVerilog-1800-2009.txt` L7007-7008: "members or elements of
  dynamic variables—class properties and dynamically sized variables—shall not
  be written with nonblocking, continuous, or procedural continuous
  assignments"), decision S14-D5; llg reports the property and the location.
  Industry-standard behaviour accepts the whole-record write (the 1800-2012
  rule) and rejects the member writes because the record holds a string.
- `neg_ref_wait_record_property_whole` and `_real` (FIN-001 R2): a task
  waiting on a record `ref` formal bound to the whole record property `h.r`,
  and one waiting on a `ref real` formal bound to `h.r.rv`, of a record with a
  string member, run with `-D N=3` and `-D N=16`. Legal (§13.5.2), rejected
  at the call: no store into the property reaches the formal's marker, and the
  wait could never resume (SIM-009).
- `neg_automatic_event_nba` and the FND-002 `neg_automatic_nba_witness`: an
  automatic task variable as an NBA target, with a repeated event control or a
  real value (SV 6.21, 13.3.2), reported by the frontend.
- `neg_automatic_in_nba_control`: an automatic variable in the event control
  of an NBA (SV 13.3.2), reported by the frontend.
- `neg_dynamic_element_nba`: a queue element as an NBA target (SV 6.21),
  reported by the frontend.
- `neg_function_event_control`: an intra-assignment event control in a
  function (SV 13.4 a), reported by the frontend.
- `neg_event_function_triggered`: `.triggered` of a call of a function
  returning an event. §15.5.3 L20733: "hierarchical_event_identifier.triggered";
  a call is not an event identifier, and industry-standard behaviour rejects
  the form too.

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

SV §7.8.6 (L8722-8723, L8735, L8743):

> If an invalid index (i.e., 4-state expression has X’s) is used during a read operation or an attempt is made to
> read a nonexistent entry, then a warning shall be issued; and the default initial value for the array type shall

> be returned, as shown in Table 7-1. A user specified default shall not issue a warning and returns the value

> 4-state integral type                  ’X

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
`S14-D6_wait_order_error_ends_run` (a failing wait_order without a fail
statement is a run-time error that ends the simulation at once, without
final procedures).

## Waits on storage with no change marker

`reject_wait_call_result`, `reject_wait_automatic_record` and
`reject_event_automatic_record` name storage whose changes llg cannot observe:
a property selected through a call result, and a member of an automatic record
with a string member that a fork shares and that something stores into inside
a fork branch, or anywhere once a `join_none` or `join_any` fork names it (one
value per activation, no change marker). A wait that could never resume rejects
with its location. `wait_container_records` checks the neighbours that stay
legal, and so does `wait_automatic_record_unchanged`: every store into its
record is before the fork or after its join, so the waiting branch reads a
record that cannot change (`3 c reached 3`, 9.4.3 L12032-12033 below; identical
to industry-standard behaviour). 9.4.2
L11837-11838 of `SystemVerilog-1800-2009.txt`:

    "Changing the value of object data members, aggregate elements, or the size
    of a dynamically sized array referenced by a method or function shall cause
    the event expression to be reevaluated."

## Waits on members of records with a string member (FIN-001 R0, S07-D18)

An unpacked structure with a string, real, handle or container member is one
runtime value when it is a class property or a static subroutine variable. A
store into it toggles the record's contents marker; a waiter evaluates its
expression again and resumes when the value changed. The clauses, all in
`SystemVerilog-1800-2009.txt`:

- 9.4.2 L11807: "An implicit event shall be detected on any change in the value
  of the expression. An edge event shall be detected only on the least
  significant bit of the expression."
- 9.4.2 L11822-11824: "The variable can be either a simple variable or a ref
  argument (variable passed by reference); it can be a member of an array,
  associative array, or object (class instance) of the aforementioned types."
- 9.4.3 L12032-12033: "The wait statement shall evaluate a condition; and, if
  it is false, the procedural statements following the wait statement shall
  remain blocked until that condition becomes true before continuing."

| Fixture | Expected output |
| --- | --- |
| `wait_record_property` | `h.r.a = 1` at 5 completes `wait (h.r.a == 1)`. `h.r.s = "s"` at 10 resumes `@(h.r.s)` and not `@(h.r.a)`, whose value did not change (L11807). `h.r.a = 2` at 15 resumes `@(h.r.a)`. The push into the queue of record element `m.e[k]` at 20 completes the wait on its size. Identical to industry-standard behaviour. |
| `event_record_property` | Through the chain `d.c.r`: `posedge` at 1 (0 to 1, L11807 least significant bit); the store into `s` at 2 and into another object's record at 3 resume nothing; `negedge` at 4; `x` changes at 6; `a or s` at 7 (the store into `s`); `posedge` at 8 (2 to 3); `d.c = other` at 9 rebinds the handle, and `other.r.a` is 1 where the old object's was 3, so `@(d.c.r.a)` resumes with 1. Identical to industry-standard behaviour. |
| `method_wait_record_property` | A method waits on its own record by bare name (1) and through `this` (2, after `set(4)` stores from another method); a derived class's method waits on the inherited record and the store goes through a base-class handle (3). Identical to industry-standard behaviour. |
| `callee_wait_record_property` | L11837-11838 above (decision B2): `ready()` reads `r.s` through its receiver, `has(c, v)` reads `c.r.a` through a handle formal and `both` calls both. `h.r.a = 7` at 1 makes `has(h, 7)` true and changes it (`has=1 event=1`); `h.r.s = "go"` at 2 makes `ready()` and `both(h)` true (`ready=2 both=2`). Each waiter records its own time, so same-step order does not show. Some simulators follow only a call's arguments and never resume these waits. |
| `ref_wait_record_property_member` (FIN-001 R2) | §13.5.2, §9.4.2: a task's `@(x)` on a `ref int` formal bound to `h.r.a` resumes at 2 when `h.r.a = 7`; the stores into `h.r.b`, `h.other`, `h.r.arr[1]` and `h.r.s` at 1 change other storage and resume nothing. A store through another formal bound to the same member (3), a `wait (x == 9)` that the first store (`8`) does not complete (5), a method waiting on its own `r.a` (6), an alias handle (7) and a virtual-interface method (8) resume too. Run with `-D N=3` and `-D N=16`. Agrees with industry-standard behaviour. |
| `ref_wait_container_property_element` (FIN-001 R2) | The same for `ref int` formals bound to `c.fa[i]`, `c.q[i]`, `c.d[i]`, `c.pa[i].a`, `c.sa[k].arr[N-1]` and `c.sq[k].a`, and through a virtual-interface method: each resumes only at the second store, into its own element; the first store into another element or member (and a `push_back`) resumes nothing. Run with `-D N=3` and `-D N=16`. Agrees with industry-standard behaviour for the array, queue and dynamic-array forms; it does not support the record-array member forms. |
| `record_property_stores` | One line per store: member and whole-record assignment (1, 2), an output record and an output member (3, 4: 13.5 L18754 "output // copy value out at end"), `++` and `+=` (5, 6), a string concatenation (7), `$sscanf` and `$sformat` targets (8, 9), a method of the object (10), the real member (11), stores through `ref` formals inside time-consuming tasks, seen when they are made (13, 14 the record; 16 an `int` member; 18 the string member: 13.5.2 L19140-19141 "changes are seen outside the subroutine immediately (before the subroutine returns)"), an assignment pattern (19) and the same pattern again (20: no value changes, no line). Identical to industry-standard behaviour. |
| `static_record_waits` | A static task's record, named from the module by its hierarchical name: the task's own stores (1, 2), member stores (3, 4), `ref` actuals (6, 8) and a pattern (9); the `wait`s complete at 3 and 8. Identical to industry-standard behaviour. |
| `static_record_nba_waits` | 4.9.4 L3536-3539 (a nonblocking assignment "always computes the updated value and schedules the update as an NBA update event"): each nonblocking store resumes the waiter in its own step (1, 2, 3). At 4 two nonblocking stores leave `a` at 7, which it already was: no line (decision T1-D11). At 5 only `x` changes. `a <= #2 9` issued at 6 commits at 8, after the line at 7 still shows 7. `always @*` (9.4.2.2 L11892-11894: "adding all nets and variables that are read by the statement") keeps `y` at `a + 1`: 8, then 10. Some simulators reject nonblocking stores into such a record, so this output follows the text. |
