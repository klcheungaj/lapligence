# SIM-035 ordinary sampled-value functions and complex clocks

IEEE 1800-2009 §§16.5-16.6, 16.9.3, 9.4.2 and 14.12-14.14 supply the oracles.
Every positive source runs through the public CLI in both optimizer modes on the
legacy and compact (portable and GMP) value backends; `complex_clocks` also runs
with `--model-opt-level O0` and after the frontend snapshot and owned Db are
destroyed. All `.out` files are derived by hand from the clauses and the
timelines below, never captured from llg.

Rules used throughout (§16.9.3):

- A sampled value is the value in the Preponed region of a time step, so a
  write in a step is visible to sampled reads only from the next step on.
- `$past(e, k, g, ev)` returns the sample of `e` in the k-th time step strictly
  before the calling step in which `ev iff g` occurred. A tick in the calling
  step is never counted; several ticks in one step count once. With fewer than
  k such steps it returns `e` evaluated on initial values: a variable's
  declaration value, otherwise its type default (X for `logic`, z for a net).
- `$rose/$fell/$stable/$changed(e, ev)` compare the Preponed value of the
  calling step with the sample of the most recent strictly earlier step in
  which `ev` occurred, or with the initial value before the first tick. `$rose`
  and `$fell` use the least significant bit (a change to 1 or to 0).
- An `iff` qualifier and the `$past` gating expression are evaluated with
  current values when the clocking event occurs (§9.4.2.3 "evaluated when a
  changes"; §16.9.3 "the event ev iff expression2"). Only `$sampled`, the
  sample expression and assertion operands read Preponed values.

## Positives

### `procedural_history` (A01, A02)

`v` is declared `4'h3`, `u` has no initializer (X), `w` is a net driven by `v`,
`en` starts at 1. Posedges of `clk` occur at 2, 5, 7 and twice at 9; `en` is
set to 0 in the step of the edge at 5, before that edge, and back to 1 at 6.
Preponed `v` at each edge: 2: 5, 5: 6, 7: 7, 9: 7 (`v = 8` follows the edges at
9). The edge at 5 fails `iff en` (current `en` is 0 although its sampled value
is 1), so the gated clock ticks at 2, 7 and 9 only.

| Line | Time | Values |
| --- | --- | --- |
| `Z` | 0 | No tick yet: `$past(v)` = 3 (declaration), `$past(u)` = x, `$past(w)` = z (net default); `$rose(v[0])`: Preponed `v` = 3 against initial 3 gives 0; `$stable(v)` = 1; `$sampled(u)` = x (the write `u[0] = 1` at 0 is not Preponed) |
| `S` | 0 | `$sampled(v)` = 3, live `v` = 5 |
| `E2` | 2 | edge step: `$past(v)` and gated `$past` have no earlier tick: 3, 3 |
| `A` | 4 | between edges: `$past(v,1)` = 5 (tick 2), `$past(v,2)` = 3 (initial); Preponed `v` = 6 against 5: changed 1, rose 0 (LSB 0), fell 1 (LSB 1 to 0); `$sampled(v)` = 6; `$past(u)` = sample at 2 = `xxx1` |
| `E5` | 5 | `$past(v)` = 5 (tick 2); gated: tick 2, so 5 |
| `E7` | 7 | `$past(v)` = 6 (tick 5); gated: 5 was gated off, so tick 2: 5 |
| `B` | 8 | ungated ticks 7, 5, 2 then initial: 7 6 5 3; gated ticks 7, 2 then initial: 7 5 3; `$stable(v)`: Preponed 7 against tick 7: 1 |
| `E9` | 9 | woken once by the first edge at 9; `$past(v)` = 7 (tick 7); gated 7 |
| `C` | 10 | ticks 9 (counted once), 7, 5: 7 7 6; Preponed `v` = 8 against tick 9 (7): changed 1 |

A twice-counted step 9 would give `C 7 7 7 1`; a sampled gate would give
`E7 6 6`; comparing the two newest ticks between edges
would give `A 5 3 1 0 0 6 xxx1`.

### `complex_clocks` (A01)

`a = 0, b = 1, c = 0, k = 0, g = 0, v = 0, r = 0.25` initially. The initial
process writes: 1: `v = 1`; 2: `a = 1`; 3: `v = 2, r = 2.5`; 4: `b = 0`;
5: `c = 1`; 6: `v = 3`; 7: `a = 0`; 8: `-> e`; 9: `v = 4`; 10: `k = 1`;
11: `k = 0, g = 1, v = 5`; 12: `k = 1`; 13: `k = 0, v = 6`; it prints at 14.
Preponed `v` at the event steps: 2: 1, 4: 2, 5: 2, 7: 3, 8: 3, 10: 4, 12: 5;
at 14 it is 6.

| Clock | Ticks | Printed values at 14 |
| --- | --- | --- |
| `posedge a or negedge b` | 2, 4 | `$past` 1..3: 2 1 0; changed (6 against 2): 1 |
| `posedge (a \| ~b)` | 2 (0 to 1; it stays 1 at 4 and 7) | 1 0 |
| `@(c)` | 5 | 2 0 |
| `edge a` | 2, 7 | 3 1 |
| `@(e)` | 8 | 3; stable (6 against 3): 0 |
| `@(cbk)`, `cbk @(posedge k)` | 10, 12 | 5 4; rose of `v[0]` (6 has LSB 0): 0 |
| `@(cbi)`, `cbi @(posedge k iff g)` | 12 (`g` is 0 at 10) | 5 0 |
| `posedge k iff g`, and `posedge k` gated by `g` | 12 | 5 0 5 0 |
| `posedge k or posedge a` gated by `g` | 12 (`g` is 0 at 2 and 10) | 5 0 |
| global `gc @(posedge a or negedge b)` | 2, 4 | `$past_gclk` 2; `$changed_gclk` 1 |

`sub` has `default clocking dc @(posedge p or posedge q)` with `p = a`,
`q = k`, `s = v`: ticks 2 (1), 10 (4), 12 (5). At 15: `$past` 1..3 = 5 4 1 and
`$fell(s[0])` (Preponed 6, LSB 0, against tick 12's 5, LSB 1) = 1.
`sub_named` declares `clocking dk @(posedge q)` and `default clocking dk;`
separately: at 16, ticks 12 and 10 give 5 4.

### `assertion_history` (A01, overlapping assertions and actions)

`clk` rises at 5, 15, 25, 35, 45; each edge's NBA increments `v`, so Preponed
`v` is 0, 1, 2, 3, 4. All
`$past(v, ...)` calls in the five assertions and their actions share one
history of depth 3.

- `a1`: `$past(v,2) == v - 2` whenever `v >= 2` holds at 25, 35 and 45.
- `a2`: `$past(v,1)` is 0, 0, 1, 2, 3, so it fails at 35. The action runs in
  the Reactive region after that edge's NBA: live `v` = 4, `$sampled(v)` = 3,
  `$past(v)` = 2, `$past(v,3)` = tick 5's 0, `$rose(v[0])` (3 against tick
  25's 2) = 1.
- `a3`: `$past(v,3)` is 0 until 45, where it is tick 15's 1; the action prints
  `past3 = 1`, `past1 = 3`.
- `a5` is disabled while `rst` is 1 (until 12) and passes afterwards.

### `event_gate_instant` (`$past` gate of a waiting-process clock)

§16.9.3 (`SystemVerilog-1800-2009.txt` L22767-22768): "the particular time
step is the kth strictly prior time step in which the event ev iff
expression2 occurred"; §9.4.2.3 (L11988): the qualifier "is evaluated when a
changes and not when enable changes". Each event below is raised while `g` is
1 and `g` is cleared later in the same statement sequence, before the clock's
waiting process can run.

- `L` at 4: `posedge a` at 2 (g = 1 when `a` rises) is a tick of the event
  list `posedge a or posedge b`, whose sample of `v` at 2 is 2. `posedge
  (a & c)` has not occurred (c = 0), so it returns the initial 1. `L 2 1`.
- `N` at 8: `->ev` at 6 and the rise of `a & c` at 6 (c set before g is
  cleared) are both ticks; the sample of `v` at 6 is 5. `N 5 5`.
- `G` at 10: `->ev` at 9 after `g = 1` ticks with the sample 7. `G 7`.

### Witnesses

- `expression_clock_witness` (FND-002 L-F12-11-02, source unchanged except
  its expectation comment): the posedge of `a | b` at 1 is the first tick, so
  `$past(x,1,,@(posedge (a|b)))` in that step has no strictly earlier tick and
  returns the initial value of `x`, its declaration value 7. The draft
  expectation 0 contradicted §16.9.3 and was not adopted.
- `real_sample_witness` (L-F12-11-03): `$sampled(r)` at 0 after `r = 2.5`
  returns the Preponed 1.25.

## Negatives

| Fixture | Rule | Diagnostic |
| --- | --- | --- |
| `neg_rose_real`, `neg_fell_shortreal`, `neg_real_assertion`, `neg_real_past`, `neg_stable_realtime` | §16.6.1 (`SystemVerilog-1800-2009.txt` L21575-21576): "The following types are not allowed: — Noninteger types (shortreal, real, and realtime)" | llg: "of a real expression is illegal" |
| `neg_string_arg`, `neg_sampled_string`, `neg_class_arg`, `neg_event_arg`, `neg_dynamic_arg`, `neg_assoc_arg` | §16.6.1 excludes these operand types | llg: illegal operand type |
| `neg_automatic_arg` | §16.9.3: argument variables shall be static | llg |
| `neg_past_zero`, `neg_past_negative`, `neg_past_nonconst`, `neg_past_unknown_ticks` | §16.9.3: `number_of_ticks` is a constant of at least 1 | frontend |
| `neg_no_clock` | §16.9.3: a clock must be explicit or inferred | frontend |
| `neg_unpacked_arg`, `neg_unpacked_struct_arg`, `neg_queue_arg` | legal or non-excluded types without sampled storage | llg: explicit unsupported |
| `neg_future_global` | §16.9.4 future functions belong to ADV-014 | llg: explicit unsupported |
