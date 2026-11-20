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
  declaration value, otherwise its type default (X for `logic`, z for an
  undriven net; a driven net holds X until its drivers first evaluate, T0-D3).
- `$rose/$fell/$stable/$changed(e, ev)` compare the Preponed value of the
  calling step with the sample of the most recent strictly earlier step in
  which `ev` occurred, or with the initial value before the first tick. `$rose`
  and `$fell` use the least significant bit (a change to 1 or to 0).
- An `iff` qualifier and the `$past` gating expression are evaluated with
  current values when the clocking event occurs (§9.4.2.3 "evaluated when a
  changes"; §16.9.3 "the event ev iff expression2"); the event occurs at the
  check that ends the writing turn (decision T1-D11). Only `$sampled`, the
  sample expression and assertion operands read Preponed values.

## Positives

### `procedural_history` (A01, A02)

`v` is declared `4'h3`, `u` has no initializer (X), `w` is a net driven by `v`,
`en` starts at 1. Posedges of `clk` occur at 2, 5 and 7; `en` is set to 0 in
the step of the edge at 5, before that edge, and back to 1 at 6. The four writes
at 9 leave `clk` at 1, its value before the turn, so the check sees no change
and step 9 has no edge (decision T1-D11). Preponed `v` at each edge: 2: 5,
5: 6, 7: 7. The edge at 5 fails `iff en` (current `en` is 0 although its
sampled value is 1), so the gated clock ticks at 2 and 7 only.

| Line | Time | Values |
| --- | --- | --- |
| `Z` | 0 | No tick yet: `$past(v)` = 3 (declaration), `$past(u)` = x, `$past(w)` = x (a driven net holds X until its drivers first evaluate, and its time-0 sample is that value, decisions T0-D3 and T0-D5; some simulators give the driver's value 3 for a net that only procedural code samples; 6.7 L5199-5200 "Nets with drivers shall assume the output value of their drivers"); `$rose(v[0])`: Preponed `v` = 3 against initial 3 gives 0; `$stable(v)` = 1; `$sampled(u)` = x (the write `u[0] = 1` at 0 is not Preponed) |
| `S` | 0 | `$sampled(v)` = 3, live `v` = 5 |
| `E2` | 2 | edge step: `$past(v)` and gated `$past` have no earlier tick: 3, 3 |
| `A` | 4 | between edges: `$past(v,1)` = 5 (tick 2), `$past(v,2)` = 3 (initial); Preponed `v` = 6 against 5: changed 1, rose 0 (LSB 0), fell 1 (LSB 1 to 0); `$sampled(v)` = 6; `$past(u)` = sample at 2 = `xxx1` |
| `E5` | 5 | `$past(v)` = 5 (tick 2); gated: tick 2, so 5 |
| `E7` | 7 | `$past(v)` = 6 (tick 5); gated: 5 was gated off, so tick 2: 5 |
| `B` | 8 | ungated ticks 7, 5, 2 then initial: 7 6 5 3; gated ticks 7, 2 then initial: 7 5 3; `$stable(v)`: Preponed 7 against tick 7: 1 |
| `C` | 10 | ticks 7, 5, 2: 7 6 5; Preponed `v` = 8 against tick 7 (7): changed 1 |

An edge seen per write at 9 would add `E9 7 7` and give `C 7 7 6 1`; a sampled gate would give
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
| `posedge a or negedge b` | 2, 4 | `$past` 1..3: 2 1 0; changed (6 against 2): 1; real `r` at 4: 2.50, at 2: 0.25, stable (Preponed 2.5 at 14 against 2.5): 1 |
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

`clk` rises at 5, 15, 25, 35, 45; each edge's NBA increments `v` and adds 0.5
to `r`, so Preponed `v` is 0, 1, 2, 3, 4 and `r` 0.0, 0.5, 1.0, 1.5, 2.0. All
`$past(v, ...)` calls in the five assertions and their actions share one
history of depth 3.

- `a1`: `$past(v,2) == v - 2` whenever `v >= 2` holds at 25, 35 and 45.
- `a2`: `$past(v,1)` is 0, 0, 1, 2, 3, so it fails at 35. The action runs in
  the Reactive region after that edge's NBA: live `v` = 4, `$sampled(v)` = 3,
  `$past(v)` = 2, `$past(v,3)` = tick 5's 0, `$rose(v[0])` (3 against tick
  25's 2) = 1.
- `a3`: `$past(v,3)` is 0 until 45, where it is tick 15's 1; the action prints
  `past3 = 1`, `past1 = 3`.
- `a4`: `$changed(r)` compares 0.0 with the initial 0.0 at 5 and fails once.
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

### Array cells (`element_past`, `element_assertion`, `element_selects`, `element_types`)

§16.5 (`SystemVerilog-1800-2009.txt` L21447): "All variables in a concurrent
assertion use the value sampled in the Preponed region of a time slot";
§16.6.1 (L21595-21596): "Fixed-size arrays, packed or unpacked, can be used as
a whole or as part-selects or as indexed bit-selects or part-selects. The
indices can be constants, parameters, or variables."; §7.4.6 (L8235-8237): "If
an index expression is out of the address bounds or if any bit in the address
is X or Z, then the index shall be invalid. The result of reading from an
array with an invalid index shall return the default uninitialized value for
the array element type." A cell selection is therefore evaluated on the
Preponed values of the cell and of its index expressions (decision S35-D1).
Every expected line below was also confirmed on an industry-standard
simulator.

`element_past`: `clk` rises at 5, 15, ..., 65 and falls at 10, ..., 60.
Writes: 7 `arr[1] = 9`; 17 `i = 2`, `en = 0`; 27 `arr[2] = c`, `en = 1`,
`ix = x`; 37 `ix = 7`; 47 `ix = z`; 57 `ix = 3`.

| Tick | sampled `arr[i]` | sampled `arr[ix]` | `en` at the edge |
| --- | --- | --- | --- |
| 5 | `arr[1]` = 2 | 2 | 1 |
| 15 | 9 | 9 | 1 |
| 25 | `arr[2]` = 3 | 9 (`ix` still 1) | 0 |
| 35 | c | x (X index) | 1 |
| 45 | c | x (7 is out of range) | 1 |
| 55 | c | x (Z index) | 1 |
| 65 | c | `arr[3]` = 4 | 1 |

Both expressions start from the initial value 2 (`arr[1]` with `i = ix = 1`).
`p1`/`p2`/`p3` are the 1st/2nd/3rd strictly prior samples of `arr[i]`, or 2;
the gated `pg` skips tick 25, where the current `en` is 0 (§16.9.3); `px` is
the prior sample of `arr[ix]`. At 60 (a negedge step): `e` = the posedge tick
55's c; `n` = the second strictly prior negedge (40), where `ix` = 7: x; the
Preponed `arr[2]` = c against tick 55's c gives `$rose` of bit 0 0, `$fell` of
bit 2 0 and `$changed` 0; `$stable(arr[ix])` compares Preponed `arr[3]` = 4
with tick 55's x: 0. `plain[1]` = 6 is read procedurally and keeps no image
(`component_only_sampled_arrays_keep_a_cell_image`).

`element_assertion`: Preponed values at the ticks 5, 15, 25, 35, 45, 55: `i` =
1, 2, 2, 2, 2, 2 (written at 12); `arr[i]` = 2, 3, 4, 3, 3, 4 (writes of
`arr[2]` at 22, 32 and 48); `arr[3]` = 4, 5, 6, 7, 8, 9 (each tick's
blocking increment comes after the Preponed sample); `arr[0]` = 1, 2, 3, 4,
5, 6 (NBA increments). `a1` (`arr[i] == 2`) passes only at 5. `a3`
(`$rose(arr[i][0])`, bit 0 = 0, 1, 0, 1, 1, 0 against the initial 0) passes at
15 and 35. `a4`, a procedural assertion after the write of `arr[3]`, fails at
25 only. `a5` fails at 25 (attempt 15: 2 < 4, then 3) and at 35, 45 and 55
(`arr[0]` not below 4). `a2`'s antecedent holds at 15, 35 and 45; the attempts
of 15 and 45 see 4 at the next tick and pass, and the attempt of 35 is
disabled by `dis[1]` (1 from 32 to 38; §16.6 L21561: "The expressions in a
disable condition are evaluated using the current values of variables (not
sampled)"), so `a2` never fails. Each line prints at the following negedge (1 = pass
action, 2 = fail action, 0 = none in that cycle; `a2` and `a5` have fail
actions only).

`element_selects`: ticks 5, 15, 25, 35, 45. Preponed values: `arr` = {11, 22,
33, 44}, {11, 23, 33, 44}, {11, 24, 33, 44}, {12, 00, 33, 44}, {13, 00, 33,
44} (each tick increments `arr[i]`; `arr[1] = 00` at 32); `i, j` = 1, 2 until
22, then 0, 1; `b` = 4 until 12, then 0; `m[1][2]` = a from 12, `m[0][1]` = f
from 42. Samples of `m[i][j]`: 5, a, 1, 1, f; `arr[i][5:2]`: 8, 8, 4, 4;
`arr[i][b]`: 0, 1, 1, 0; `arr[j][i*2 +: 3]`: 4, 4, 4, 0; bit 0 of `arr[i]`:
0, 1, 1, 0, 1; bit 1 of `arr[j]`: 1, 1, 0, 0, 0; `arr[i][7:4]`: 2, 2, 1, 1, 1,
each with the time-0 value as its initial value (5, 8, 0, 4, 0, 1, 2). `$past`
prints the previous sample; the value change functions compare the current
sample with the previous one. `$sampled(arr[j])` is the Preponed cell (33, 33,
24, 00, 00); after the blocking increment `$sampled(arr[i])` is still the
Preponed cell and the live cell is one more.

`element_types`: ticks 5, 15, 25, 35. `ra[0]` is Preponed 0.5, 1.5, 2.5 and
0.25 (written at 32) and is incremented after the sample; `i` is 1 until 12,
then 0, so `$sampled(ra[i])` is 1.5, 1.5, 2.5, 0.25 and `a1` (`ra[i] > 1.0`)
fails only at 35. `w[1]` is a net driven by `src[1] ^ f`: it holds X until its
driver first evaluates (T0-D3), so its time-0 initial value is x, then d, and
f from 7. `st[1]` is B (1) until 7, D (3) until 27, then B, so `$past(st[k])`
is 1, 1, 3, 3 and `a2` (`st[k] != D`) fails at 15 and 25. `pr[1]` is 34 until
7, then 39; its member `lo` 4, then 9. Each process line prints in the Active
region of the tick and each action in the Reactive region after it.

### Real values (`real_past`, `real_element_past`, `real_past_shortreal`)

§16.6.1 (`SystemVerilog-1800-2009.txt` L21575-21576): "The following types
are not allowed: — Noninteger types (shortreal, real, and realtime)". By user
decision AA-D8 llg serves `$past`, `$stable` and `$changed` of real values as
industry-standard behaviour does; the samples follow §16.9.3 (L22767-22772):
"$past returns the value of expression1 that was sampled in the Preponed region
of a particular time step strictly prior to the one in which $past is
evaluated. [...] the value returned from the $past function is the result of
evaluating expression1 using the initial values of the variables comprising
it." `$stable` and `$changed` compare the samples bit for bit. `real_past` and
`real_element_past` were also confirmed on an industry-standard simulator;
`real_past_shortreal` was not, because that simulator stops with an internal
error on any sampled value function of a shortreal.

`real_past`: `clk` rises at 5, 15, 25, 35, 45; each edge's NBA adds 0.5 to `r`,
1.0 to `a` and `t`, 1 to `i` and toggles `en`. Preponed `r` is 0.25, 0.75,
1.25, 1.75, 2.25, so `$past(r, 1..3)` is the 1st..3rd strictly prior sample or
the declaration value 0.25. The gate `en` is 0, 1, 0, 1, 0 when each edge
occurs (current value), so only 15 and 35 are gated ticks: `g` = 0.25, 0.25,
0.75, 0.75, 1.75. `u` has no initializer: 0.0 (Table 6-7). `a * 1.5` samples
1.5, 3.0, ...: its `$past` is 1.5, 1.5, 3.0, 4.5, 6.0. The process reads the
current `r` before the NBA, which equals its sample: `eq` 1 only at 5, `lt` 0
only at 5. `t` (realtime) samples 2..6: `$past` 2, 2, 3, 4, 5. Packed
arguments that read reals keep packed histories: `int'(r)` samples 0, 1, 1, 2,
2 (rounded): `ci` 0, 0, 1, 1, 2; `i + r` samples 3.25, 4.75, ...: `sum` 3.25,
3.25, 4.75, 6.25, 7.75; `r > 1.0` samples 0, 0, 1, 1, 1: `cmp` 0, 0, 0, 1, 1 and
`$stable` 1, 1, 0, 1, 1. At 5 the immediate `a_proc` (`$past(r) != r`), `a_mod`
(`$past(r) < r`), `a_eq` (`$past(r, 2) + 1.0 == r`) and `a_t` (`$changed(t)`
against the initial 2.0) fail; `a_eq` also fails at 15 (1.25 against 0.75);
its action prints the current `r` after the NBA. At each negedge the
explicit-clock `$past(r, 1, , @(posedge clk))` returns the previous posedge's
sample: 0.25, 0.75, 1.25, 1.75.

`real_element_past`: ticks 5, 15, ..., 75. Preponed `i` is 1..7, then 0; each
tick adds 10.0 to `ra[i]` (no write for 4..7, out of range). Preponed `ra[i]`
is 1.5, 2.5, 3.5, then 0.0 (§7.4.6 L8235-8237: an invalid index reads the
default) from 35 to 65 and `ra[0]` = 0.5 at 75; `c1..c3` are its 1st..3rd prior
samples or the initial 1.5. `ix` = `x1` is always invalid: 0.00. `rz` has no
initializer; `rz[1]` is written 1 at 15 and 4 at 45, so its `$past` is 0, 0, 0,
1, 1, 1, 4, 4. `j` samples 0, 0, 1, 1, ...; `rs[1]` becomes 7.0 at 25, so
`rs[j]` samples 0.5, 0.5, 1.5, 7.0, 7.0, ...: `$stable` 1, 1, 0, 0, 1, ... and
`$stable(rs[1])` is 0 only at 35. `a_cell` and `a_ch` never fail.

`real_past_shortreal`: `s` doubles at each edge from 1.5: samples 1.5, 3, 6, 12,
`$past` 1.5, 1.5, 3, 6; `$stable` 1 only at 5, where `a_s` fails. `sa[1]` is
written 4.5 at 15: samples 0.75, 0.75, 4.5, 4.5, `$past` 0.75, 0.75, 0.75, 4.5,
`$changed` 1 only at 25.

### `element_burst` (descriptor-form cells)

The same §16.5 and §16.6.1 lines apply to a 5000-cell array, which llg keeps
in descriptor form and samples by saving each cell's old value at its first
change in a time slot. `clk` rises at 5, 15, 25, 35 and 45; the tick at 5 sets
`arr[k]` to `k` mod 256. At 15 a blocking loop increments every cell after the Preponed
sample, so the sampled cells are still 00, 64 (100) and 87 (4999 mod 256 =
135) while the live cells are 01 and 88. At 25 the Preponed values are the
incremented ones (`arr[2]` = 03, `arr[0]` = 01, `arr[4999]` = 88), not the
values saved at 15; `arr[2]` becomes f2 after the sample. At 35 the sample of
`arr[2]` is f2 and it becomes f3; at 45 its sample is f3. `$past` returns the
sample of the previous tick: `arr[0]` 00 at 25 and 01 at 35; `arr[2]` 02, 03
and f2 at 25, 35 and 45.

### `element_burst_dense` (dense cells after a burst)

The `element_burst` design and lines with a 4096-cell array, the largest that
llg keeps in directly addressed storage with a Preponed cell image, which it
updates at each Preponed sample from the cells written since the previous
one. `arr[4095]` is 4095 mod 256 = ff at 15 (live 00 after the increment);
00 at 25 and 35. The other values are those of `element_burst`.

### `element_burst_shrink` (descriptor cells over many slots)

The same §16.5 and §16.6.1 lines, and §16.9.3 (L22767-22768): "$past returns
the value of expression1 that was sampled in the Preponed region of a
particular time step strictly prior to the one in which $past is evaluated."
The index is part of `expression1`, so `$past(arr[cyc - 1])` reads cell
`cyc - 2` with the values of the previous tick. `clk` rises at 5 + 10c for
tick c (`cyc` = c at its Preponed sample). Tick 0 sets `arr[k]` to `k` mod
256; tick c in 1..149 sets `arr[c]` to c0 ^ c after the sample, so at tick c
the sample of `arr[c]` is c mod 256 and `$past(arr[c - 1])` is c0 ^ (c - 2):
c49 31, ef, live f1; c99 63, a1, live a3; c149 95, 53, live 55; `arr[0]`
stays 00. Tick 150 inverts every cell after the sample: samples c1 (`arr[1]`
= c0 ^ 01), 55 (`arr[149]`) and 87 (4999 mod 256), live 3e and 78. At 1515
the samples are 3e, aa and 78 and `$past(arr[1])` is c1. Over ticks 1-149
llg's table of saved values shrinks twice and regrows at tick 150.

### `element_write_paths` (dense cells written every way)

§16.5 and §16.6.1 as above, and §10.6.2 (L13390-13393): "When released, then
if the variable is not driven by a continuous assignment and does not
currently have an active assign procedural continuous assignment, the
variable shall not immediately change value and shall maintain its current
value until the next procedural assignment to the variable is executed."
Four arrays keep a cell image (`arr`, `other`, the net array `w` and the real
array `ra`). Time 0: `arr[k]` = k, `other` = 10 11 12 13 (declaration),
`ra[k]` = k, `w[0]` = `arr[0]` ^ ff = ff, `w[1]` = `other[1]` = 11. Each
tick prints the samples, then (`after`) the samples and live values of
`arr[2]`, `arr[5]`, `arr[7]` once its writes are done, so the samples stay
the Preponed values within the tick.

- 5 (c0): initial values; then `arr[1]` = a1, `arr[2]` = b2.
- 15 (c1): a1 and b2 sampled; then bit 0 of `arr[3]` cleared (02), bits 7:4
  of `arr[4]` set to c (c4), `ra[2]` = 2.5, and `other[1] <= 99` from the
  second process (so `w[1]` = 99).
- 25 (c2): 02, c4, 99 and 2.5 sampled, `arr[2]` still b2 (written two slots
  earlier); then every cell of `arr` is 55, so `w[0]` = aa.
- 35 (c3): every `arr` sample 55, `w[0]` aa; then `force arr[5] = ee`,
  `put(arr[6], 66)` through a `ref` argument.
- 45 (c4): ee and 66 sampled; then `release arr[5]` keeps ee (§10.6.2),
  `arr[7]` = 77 and then 07 in the same slot.
- 55 (c5) and 65 (c6): `arr[7]` samples 07; nothing else changes.

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
| `neg_rose_real`, `neg_fell_shortreal`, `neg_rose_real_compare`, `neg_past_gclk_real`, `neg_stable_gclk_real` | §16.6.1 (`SystemVerilog-1800-2009.txt` L21575-21576): "The following types are not allowed: — Noninteger types (shortreal, real, and realtime)". A real anywhere in a `$rose`/`$fell` argument and in a global clocking function stays illegal (AA-D7); industry-standard tools reject `$rose`/`$fell` of a real (they cannot compile any sampled value function of a shortreal) and accept the global clocking forms with values that do not follow §16.9.4 | llg: "of a real expression is illegal" |
| `neg_string_arg`, `neg_sampled_string`, `neg_class_arg`, `neg_event_arg`, `neg_dynamic_arg`, `neg_assoc_arg` | §16.6.1 excludes these operand types | llg: illegal operand type |
| `neg_automatic_arg` | §16.9.3: argument variables shall be static | llg |
| `neg_past_zero`, `neg_past_negative`, `neg_past_nonconst`, `neg_past_unknown_ticks` | §16.9.3: `number_of_ticks` is a constant of at least 1 | frontend |
| `neg_no_clock` | §16.9.3: a clock must be explicit or inferred | frontend |
| `neg_unpacked_arg`, `neg_unpacked_struct_arg`, `neg_queue_arg` | legal or non-excluded types without sampled storage | llg: explicit unsupported |
| `neg_queue_element`, `neg_dynamic_element_operand`, `neg_assoc_element` | §16.6.1 (`SystemVerilog-1800-2009.txt` L21576, L21592-21593): "The following types are not allowed: [...] — Associative arrays — Dynamic arrays"; container elements have no Preponed image (industry-standard tools reject them too) | llg: located unsupported, at the selection |
