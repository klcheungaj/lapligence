# SIM-033 clocking input expressions and legal sample types

IEEE 1800-2009 §§3.14.3 and 14.3-14.15 supply the oracles. Every positive
source runs through the public CLI in both optimizer modes on the legacy and
compact (portable and GMP) value backends; `skew_timeline`, `expressions` and
`interfaces` also run after the frontend snapshot and owned Db are destroyed.
All `.out` files are derived by hand from the clauses and the timelines below,
never captured from llg.

Sampling rules used throughout (§14.13): an input with a skew other than an
explicit `#0` takes the value its signal had in the Postponed region of the
time step `skew` units before the clocking event, so `#1step` (the default,
§14.3) reads the end of the previous time step; an explicit `#0` reads the
Observed region of the event's own step, after that step's NBA updates. A
clocking input bound to an expression samples the expression's value (§14.5).
`@(cb)` resumes after the block updated its samples (§14.13); a clockvar holds
its sampled value between events and its event control sees changes of that
value (§14.15).

## Positives

### `skew_timeline` (A01)

`d` starts at 00; the initial process writes `d = 10` at 1, `d = 40` at 7,
`d = 50` at 11 and `d = 60` at 13 (before raising the clock in the same
step); `always @(posedge clk) d <= d + 1` updates `d` in the NBA region of
every edge. Posedges fall at 4, 6, 13, 15 and 40 (irregular intervals). End-of-
step values: 1:10, 4:11, 6:12, 7:40, 11:50, 13:61, 15:62, 40:63.

| Edge | `#1step` / default | `#0` | `#2` (step t-2) | `#SK = #3` (t-3) | `#2 {d[3:0],d[7:4]}` |
| --- | --- | --- | --- | --- | --- |
| 4 | end of 1: 10 | 11 | 2: 10 | 1: 10 | 01 |
| 6 | 11 | 12 | 4: 11 | 3: 10 | 11 |
| 13 | 50 (the same-step `d = 60` is later than Preponed) | 61 | 11: 50 | 10: 40 | 05 |
| 15 | 61 | 62 | 13: 61 | 12: 50 | 16 |
| 40 | 62 | 63 | 38: 62 | 37: 62 | 26 |

`d` printed after `@(cb)` already holds the NBA result, equal to the `#0`
sample.

Some simulators sample an input whose skew reaches back past the previous
clocking event at a later time (here `#SK` at 15 reads 61, the value after the
edge at 13, while the same `#3` skew reads the correct step at 6 and 13); llg
follows 14.13 L19909-19911 (`SystemVerilog-1800-2009.txt`): "If the input skew
is not an explicit #0, then the value sampled corresponds to the signal value
at the Postponed region of the time step skew time units prior to the clocking
event", which at 15 is step 12 (50).

### `expressions` (A02, concrete)

Inputs bound to `v[3:0]`, `v[7]`, `{a, b, v[1:0]}` (10 bits), the
hierarchical select `tb.u.sig[7:4]`, the packed-struct member `p.lo`, the
whole struct `p` (read back through `cb.pw.hi`), `n + a` (32-bit), the pure
function `twice(n)`, real `r` with `#1step`, `#0 r * 2.0` and `#2`, and a
shortreal. At 2 the test writes `v = 3C, a = 7, r = 2.5, u.sig = 5A,
p.hi = E, n = 7, sr = 0.75`; the edge at 4 is followed in the same step by
`v = FF, r = 9.0` (seen only by `#0`); `r = 3.25` at 6; `v = 81, b = 0` at 7;
second edge at 9.

- 4: `lo = C`, `v[7] = 0`, `{7, C, 00} = 0x1F0`, `sig[7:4] = 5`, `p.lo = 2`,
  `p = E2`, `7 + 7 = 14`, `2 * 7 = 14`, `r = 2.5`, `#0`: `9.0 * 2 = 18.0`,
  `#2` (step 2): `2.5`, shortreal `0.75`.
- 9: `lo = 1`, `v[7] = 1`, `{7, 0, 01} = 0x1C1`, the rest as at 4 except
  `r = 3.25`, `#0`: `3.25 * 2 = 6.5`, `#2` (step 7): `3.25`.
- `@(cb.lo)`: the clockvar starts at X and becomes C at 4; `@(posedge
  cb.bit7)` is armed after that and sees 0 to 1 at 9.

### `interfaces` (A02, interfaces)

Interface `bus` declares the clocking block (`data`, `data[3:0]`, a nibble
swap, real `level`, `#0 level * 4.0`, `#2 level`) and `modport tb_mp(clocking
cb)`. The clock toggles every 5 (posedges 5, 15, 25). `b`: `data = 3C,
level = 1.5` at 0; `data = A7, level = 2.5` at 12; `level = 7.0` in the step
of the edge at 15 (seen only by `#0`); `data = 01, level = 0.25` at 21. `b2`
keeps `data = 55, level = 9.5`.

- Program `reader` reads through its modport clocking port after each `@(p.cb)`
  (5, 15, 25): `3C C C3 1.50 6.00 1.50`, `A7 7 7A 2.50 28.00 2.50` (the `#2`
  sample is step 13), `01 1 10 0.25 1.00 0.25`.
- The module reads the concrete instance at 6 (edge-5 samples), the virtual
  interface bound to `b` at 16 (edge-15 samples), the same handle rebound to
  `b2` at 26 (`55 5 55 9.50 38.00 9.50`) and the `virtual bus.tb_mp` view of
  `b` at 26 (edge-25 samples). Program and module lines are printed at
  different times, so their order is fixed.

### `defaults`

`default clocking busA;` names a block declared earlier in the same module and
is repeated inside the nested module `inner` (§14.12). Posedges at 2, 7, 10;
negedges at 3, 9, 15; `d` increments in the NBA region of each posedge. `##1`
from time 0 ends at 2 with `busA.d = 0`; `##0` continues in the same step
because the clocking event already occurred (§14.11); the global clocking
event (negedge) next occurs at 3; `inner` waits `##3` until 10 and reads the
`#1step` sample 2.

### Adopted FND-002 witnesses (source unchanged)

`const_skew_witness` (L-F12-04-04: literal skew, value 1),
`expression_witness` (L-F12-04-02: `a + b = 7`), `real_witness`
(L-F12-04-01: 1.25), `cross_clock_cycle_witness` (L-F06-07-02, L-F12-04-05:
`##1` on the default clock, then a drive of another block's output, 7).

### Unpacked inputs (K1)

An unpacked fixed-size clockvar (fixed array or unpacked structure, any
nesting of them with integral leaves) samples its whole value into one
packed cell, its declaration-order image; element, member and slice reads
select from that cell. Expected outputs are industry-standard behaviour's
output for the same source, checked against the timelines below by hand.

- `unpacked_inputs`: `arr` is `{1, 2}`, `grid[j][k] = 3j + k`, `st = {3, 5}`,
  `recs[1] = {6, 7}` at 0; at 7 `arr[1] = 20`, `st.a = 30`, `grid[1][2] = f`,
  `recs[1].a = 66`; `arr[0] = 11` at 12; `arr = {5, 6}`, `st.b = 9` at 20;
  `arr[0] = 44` at 24; `i = 0` at 26. Posedges at 5, 15, 25, 35. The #1step
  inputs read the end of the previous step (14.13 L19909-19911), so at 15
  `arr` is `{11, 20}` and at 25 `{44, 6}`; `#2 sk` reads step 23 at 25
  (`{5, 6}`) and step 33 at 35 (`{44, 6}`); `#0 a0` reads the Observed region
  of the event's own step (L19911-19912). `cb.arr[i]` follows `i`; the
  whole-value copy, the structure comparison with its source, `foreach` and
  `$size` read the sample.
- `record_inputs`: a record with a member array of 16 cells (one column) and
  of 3 cells (one leaf per cell) prints the same; `#2 rk` at 15 reads step 13,
  after `r.m[N-1] = 88`.
- `vif_unpacked_inputs`: elements, a multidimensional element, a `#0`
  expression-bound input and a whole-array copy through a virtual interface
  follow the handle, rebound from `b1` to `b2` at 17.

### Edge input skews (K1)

§14.3 L19457-19458: "When the clocking event specifies a simple edge,
instead of a number, the skew can be specified as the specific edge of the
signal." An `input negedge x` under `@(posedge clk)` takes the `#1step`
sample at each negedge of `clk` and holds it from that edge on (decision
K1-D2, industry-standard behaviour); a skew naming the clocking event's own
edge is the `#1step` sample. Expected outputs are industry-standard
behaviour's.

- `edge_skews` (the plan's probe): `x`, `y`, `z` take 1 at 7, 2 at 12, 3 at
  22, 4 at 24; the clock toggles every 5. `cb.x` reads the negedge samples
  (10: 1, 20: 2, 30: 4) at 15, 25, 35; `posedge y` and `#1step z` read 2, 4, 4.
- `edge_skew_forms`: a negedge skew under a posedge event (four-state `w`
  is X before its first negedge, real `r` 0.0), a posedge skew under a
  negedge event, a negedge skew under `@(clk)` (at a negedge event it is the
  `#1step` sample), a block default edge skew, an unpacked edge-skewed input,
  and a read at 12, between the negedge at 10 and the posedge at 15, that
  already sees the negedge sample.

### Clocking block events through virtual interfaces (K1)

`@(vif.cb)` resumes at the clocking block event (§14.13 L19921-19922: "a
clocking block shall update its sampled values before triggering the event
associated with the clocking block name") of the instance the handle is bound
to when the wait arms. Expected outputs are industry-standard behaviour's.

- `vif_clocking_event`: the former `neg_vif_clocking_event`, through a modport
  view: the edge at 5 reads `11`.
- `vif_clocking_events`: `@(v.cn)` (a negedge block) resumes at 10; `@(v.cb
  iff en)` with `en` set at 12 at 15; `@(vp.cb or posedge en)` through a view
  of `b2` at 25 (`en` no longer rises); a class property handle bound to `b2`
  and a second wait on `v` (bound to `b1`, whose `data` became 33 at 21) wait
  in parallel, resuming at 35 and 45.
- `vif_clocking_event_null`: a wait through a null handle stops the run
  (decision S99-D8) after `0 before`.

### Shared sampled source (K1)

`shared_source_samples` reads one source through a `#1step` clockvar, an
`input #3` clockvar and two `#0` clockvars in different blocks, one of them on
`@(clk2)` firing twice per period, over six periods. It guards the runtime's
reuse of a source's history entries and of each `#0` clockvar's Observed copy
record across clocking events. The `c2` display is delayed by 2 because a
clockvar keeps its sample until the next clocking event, which keeps it from
racing the `c3` process at the same time. The expected output is
industry-standard behaviour's.

## Negatives

Language rules, rejected in both optimizer modes:

- `neg_dynamic_skew`: a skew is a constant expression (§14.4); FND-002
  witness, reported by the frontend.
- `neg_cycle_no_default`: `##` without a default clocking (§14.11); FND-002
  witness.
- `neg_compound_drive`, `neg_concat_drive`: clockvar writes are synchronous
  drives and not concatenation parts (§14.16); FND-002 `neg_clocking_compound`
  and `neg_clocking_concat`.
- `neg_input_write`, `neg_output_read`: an input cannot be driven and an output
  cannot be read (§14.3).
- `neg_output_expression`: an output clocking expression must be a legal
  output port connection (§14.5).
- `neg_automatic_source`: a clocking signal shall not designate a variable
  restricted to a procedural block (§14.3).
- `neg_unresolved_member`: no such clockvar.
- `neg_chandle_input`: ports cannot have the chandle type (§6.14), so a
  chandle cannot be a clocking input (§14.5).

Forms rejected with a located diagnostic instead of a wrong sample (K1):

- `neg_string_input`, `neg_class_input`, `neg_event_input`, `neg_queue_input`,
  `neg_unpacked_union_input`: string, class handle, event, unpacked or tagged
  union, queue, dynamic and associative array clockvars (decision K1-D1: the
  text does not exclude them, industry-standard behaviour rejects them).
- `neg_union_member_input`: an unpacked structure clockvar with an unpacked
  union member rejects like the union itself (K1-D1).
- `neg_real_array_input`: an unpacked clockvar with a real element or member
  has no packed sample image; industry-standard behaviour serves it, llg does
  not yet ([known issue](../../../../../docs/known_issues.md#clocking-input-forms-outside-the-sampled-storage-model)).
- `neg_input_both_edges_skew`: an `edge` input skew; industry-standard
  behaviour treats it as a syntax error.
- `neg_edge_skew_delay`: an edge skew with a delay (`negedge #1`), which
  industry-standard behaviour accepts and llg does not serve.
- `neg_edge_skew_event`: an edge skew under an event list; §14.3 allows an
  edge skew "When the clocking event specifies a simple edge", and
  industry-standard behaviour rejects it too.


