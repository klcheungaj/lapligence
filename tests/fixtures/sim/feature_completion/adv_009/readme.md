# ADV-009 system timing checks and notifiers

Every expected output is hand-derived from the clauses quoted below, with the
predicted times written next to each stimulus in the sources; none is produced
by llg. Every source is Verilog and runs in both editions (`--edition v2001`
and `sv2009`), in both optimizer modes and on the legacy, compact/portable and
compact/GMP value backends (`tests/sim_feature_completion/adv_009.rs`). The
outputs print notifier values and design state only; the violation reports
on stderr are checked separately. The portable decisions are the `A9-D*`
cases in `tests/fixtures/sim/lrm_decisions/` (see `docs/lrm_decisions.md`).

## Clauses

Line numbers are those of the `pdftotext -layout` extractions
`Verilog-1364-2001.txt` (V) and `SystemVerilog-1800-2009.txt` (SV).

- `$setup`, SV 31.3.1 L50983-50988 (V 15.2.1 L15857-15859): "(beginning of
  time window) < (timestamp time) < (end of time window) The end points of the
  time window are not part of the violation region. When the limit is zero,
  the $setup check shall never issue a violation." The data event is the
  timestamp, the reference event the timecheck (Table 31-1).
- `$hold`, SV 31.3.2 L51026-51028 (V 15.2.2 L15917-15919): "(beginning of
  time window) <= (timecheck time) < (end of time window)", window from the
  reference (timestamp) event to it plus the limit.
- `$removal`, SV 31.3.4 Table 31-4 L51182-51183: "reference_event Timecheck
  event", "data_event Timestamp event"; L51200-51202 (V 15.2.4
  L16106-16108): "(beginning of time window) < (timestamp time) < (end of time
  window)". V Table 52 (L16090-16092) swaps the roles; llg follows SV in both
  editions (A9-D2).
- `$recovery`, SV 31.3.5 L51254-51256 (V 15.2.5 L16167-16169): "(beginning
  of time window) <= (timecheck time) < (end of time window)".
- `$skew`, SV 31.4.1 L51438-51443 (V 15.3.1 L16368-16372): "(timecheck time)
  - (timestamp time) > limit Simultaneous transitions on the reference and
  data signals shall not cause $skew to report a timing violation, even when
  the skew limit value is zero." L51452-51456 (V L16398): "A second
  consecutive reference event shall cancel the old wait for the data event
  and begin a new one. ... $skew shall report timing violations for all data
  events occurring beyond the limit after a reference event."
- `$width`, SV 31.4.4 L51802 "data event = reference event signal with
  opposite edge"; L51818-51829 (V 15.3.4 L16733-16741): "threshold <
  (timecheck time) - (timestamp time) < limit ... no violation is reported for
  glitches smaller than the threshold. The threshold argument shall be
  included if the notifier argument is required. ... If the notifier is
  present, a non-null value for the threshold shall also be present."
  L51843-51844 lists `$width ( negedge clr, lim, , notif );` as illegal.
- `$period`, SV 31.4.5 L51884 "data event = reference event signal with the
  same edge"; L51890-51893 (V 15.3.5 L16817-16820): "(timecheck time) -
  (timestamp time) < limit".
- `$nochange`, SV 31.4.6 L51959-51962 (V 15.3.6 L16895-16897): "(beginning
  of time window) < (data event time) < (end of time window) The end points of
  the time window are not included." L51971-51972 (V L16908): "It shall not
  be a violation if posedge clk and a transition on data occur
  simultaneously."
- Edge control, SV 31.5 L52010-52027 (V 15.4 L16952): "Edge transitions
  involving z are treated the same way as edge transitions involving x",
  `posedge` is `edge[01, 0x, x1]` and `negedge` is `edge[10, x0, 1x]`.
- Notifiers, SV 31.6 Table 31-13 L52048-52054: "x Either 0 or 1; 0 1; 1 0;
  z z"; V 15.5 Table 61 L17000-17008 gives x -> 0, which llg follows in both
  editions (A9-D5).
- Conditions, SV 31.7 L52190-52197 (V 15.6 L17388-17404): "The comparisons
  used in the condition can be deterministic, as in ===, !==, ~, or no
  operation, or nondeterministic, as in == or !=. When comparisons are
  deterministic, an x value on the conditioning signal shall not enable the
  timing check. For nondeterministic comparisons, an x on the conditioning
  signal shall enable the timing check. ... if a vector net or an expression
  resulting in a multibit value is used, then the least significant bit of the
  vector net or the expression value is used."
- Vectors, SV 31.8 L52242-52244 (V 15.7 L17442-17443): "the transition of one
  or more bits of a vector is considered a single transition of that vector."

## Fixtures

- `windows.v`: `$setup`/`$hold`/`$recovery`/`$removal` with limit 10 and the
  reference (clock) edge at 100. Setup data at 89, 90 (the limit), 91, 99 and
  100 (an earlier and a later delta): only 91 and 99 violate. Hold data at 100
  (earlier delta, same delta, later delta), 101, 109 and 110: all but 110
  violate (`<=` at the start, A9-D1). Recovery clock at 99, 100 (same delta),
  109, 110 after `r` at 100: 100 and 109. Removal `r` at 100 (same delta), 101,
  109, 110 after the clock (data, timestamp) at 100: 101 and 109.
- `skew_period_width.v`: `$skew(posedge a, posedge b, 10)`: b at 95 before any
  a, at 100 with a, at 110 (the limit): none; at 111 and again at 113: both
  (x -> 0 -> 1); after a second reference at 107, b at 116 (9): none. Zero
  limit: simultaneous none, one tick late violates (101). `$period(posedge c,
  10)` from 100: 109 violates, 110 not, 111 then 115 (4) violates.
  `$width(posedge c, 10, 2)` from 100: widths 0, 1, 2 (threshold) none, 3 and
  6 violate, 10 none.
- `nochange.v`: `$nochange(posedge c, d, 0, 0)` with c high 100..119: data at
  99, at 100 with the leading edge, at 119 with the trailing edge and at 120:
  none; at 101: violation. Offsets 2 and 3 make the window (98, 122): data at
  98 none, at 99 reported at the leading edge (100), at 121 reported at 121,
  at 122 none.
- `edges.v`: setup data at 95, reference transitions at 100. `posedge`
  matches 0->x, 0->z, x->1, z->1 but not x->z or 1->x; `negedge` matches 1->z
  but not z->x; `edge[01]` matches only 0->1; `edge[0x, x1]` matches 0->z but
  not 0->1. A hold data change x->z is no event, z->1 is. A two-bit reference
  `posedge` occurs for 00->10 and 01->10 but not 11->10.
- `conditions.v`: `$setup(d, posedge c &&& cond, 10)` with the enable 0, 1,
  x, z for `en`, `~en`, `en == 1`, `en === 1`, `en != 0`, `en !== 0`,
  `en == 1'b0` (bits printed 3..0 = z, x, 1, 0). Timestamp conditions settle
  in the step of the data event (0, 1, 0, 1 enabled: two violations). Hold
  stamp conditions at the clock settle to 0, 1, 1, 0. `$width` with the
  condition true at both edges violates; false at either edge does not.
  `$nochange` conditions are read at the data event (1, 1, 0, 0).
- `notifiers.v`: a flip-flop UDP whose notifier input drives q to x; width,
  setup and hold violations at 110, 135 and 191 move the notifier x -> 0 -> 1
  -> 0 and q to x each time; the clock at 190 captures 0. Notifiers starting
  1 and z go 1 -> 0 -> 1 -> 0 and stay z; a notifier shared by two violating
  checks changes once per event; duplicate checks keep the larger limit
  (A9-D6); a time-0 data change is no timestamp (A9-D3).
  `notifiers_unchanged.out` is the output with `--no-timing-checks` or
  `--no-notifiers`; `--no-timing-check-messages` keeps `notifiers.out`.
- Negative cases (`neg_*.v`) stop before the model runs: a vector notifier,
  a `$width` notifier without threshold, a `$period` edge list with rising and
  falling transitions, a missing limit, a non-constant limit and a `$width`
  without an edge. `$setuphold`, `$recrem`, `$timeskew`, `$fullskew` and
  negative `$nochange` offsets are covered by ADV-010
  (`../adv_010/readme.md`).
