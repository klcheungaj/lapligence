# ADV-010 combined and negative timing checks

Every expected output is hand-derived from the clauses quoted below with the
rules listed under "Derivation"; the stimulus times are in the sources and the
expected windows next to each instance group. Every source is Verilog and runs
in both editions (`--edition v2001` and `sv2009`), in both optimizer modes and
on the legacy, compact/portable and compact/GMP value backends
(`tests/sim_feature_completion/adv_010.rs`). The outputs print, per checked
instance, `count@last`: how often its notifier changed and the time of the
last change (0: never). Violation reports and generation warnings on stderr
are checked separately. The portable decisions are the `A10-D*` cases in
`tests/fixtures/sim/lrm_decisions/` (see `docs/lrm_decisions.md`).

Negative limits take effect only with `--neg-timing-checks` (or
`[simulator] neg_timing_checks = true`); `*_off.out` is the output without
it and `negative_disabled.out` the one with `--no-timing-checks`.

## Clauses

Line numbers are those of the `pdftotext -layout` extractions
`Verilog-1364-2001.txt` (V) and `SystemVerilog-1800-2009.txt` (SV).

- `$setuphold`, SV 31.3.3 L51110-51112 (V 15.2.3 L16013): "When either the
  setup limit or the hold limit is negative, the restriction becomes as
  follows: setup_limit + hold_limit > (simulation unit of precision)";
  L51119: "is equivalent in functionality to the following, if tSU and tHLD
  are not negative" (a `$setup` and a `$hold`).
- `$recrem`, SV 31.3.6 L51327-51340 (V 15.2.6 L16260): "removal_limit +
  recovery_limit > (simulation unit of precision) ... `$recrem( posedge
  clear, posedge clk, tREC, tREM );` is equivalent in functionality to the
  following, if tREC and tREM are not negative: `$removal( posedge clear,
  posedge clk, tREM ); $recovery( posedge clear, posedge clk, tREC );`".
- Negative checks, SV 31.9 L52282-52285 (V 15.8 L17480): "Both the
  $setuphold and $recrem timing checks can accept negative values when the
  negative timing check option is enabled."
- SV 31.9.1 L52377-52379 (V 15.5.1 L17118-17124): "A timing violation shall
  be triggered if the signal changes in the violation window, exclusive of
  the end points. Violation windows smaller than two units of simulation
  precision cannot yield timing violations."
- SV 31.9.1 L52418-52427: "only one implicit delayed signal is created, and
  it is used for both timing checks. ... CLK has only one delayed signal
  created for it, del_CLK, rather than one explicit delayed signal for the
  first check and another implicit delayed signal for the second check."
- SV 31.9.1 L52432-52441 (V L17185): "The delayed versions of the signals,
  whether implicit or explicit, shall be used in the $setup, $hold,
  $setuphold, $recovery, $removal, $recrem, $width, $period, and $nochange
  timing checks; and these checks shall have their limits adjusted
  accordingly so that the notifier shall be toggled at the proper moment. If
  the adjusted limit becomes less than or equal to 0, the limit shall be set
  to 0, and the simulator shall issue a warning. The delayed versions of the
  signals shall not be used for the $skew, $fullskew, and $timeskew timing
  checks".
- SV 31.9.1 L52444-52449 (V L17208-17212): "It is possible for a set of
  negative timing check values to be mutually inconsistent and produce no
  solution for the delay values of delayed signals. In these situations, the
  simulator shall issue a warning. The inconsistency shall be resolved by
  changing the smallest negative limit value to 0 and recalculating the
  delays for the delayed signals, and this shall be repeated until a solution
  is reached."
- SV 31.9.1 L52451 "The delayed timing check signals are only actually
  delayed when negative limit values are present."; L52460-52466: "(CLK = Q)
  = 6; $setuphold (posedge CLK, posedge D, -3, 8, , , , dCLK, dD); $setuphold
  (posedge CLK, negedge D, -7, 13, , , , dCLK, dD); The setup time of -7 (the
  larger in absolute value of -3 and -7) creates a delay of 7 for dCLK".
- SV 31.9.2 L52499-52507 (V 15.5.2 L17232): "timestamp_condition is
  associated with the delayed signal that transitions first, while
  timecheck_condition is associated with the delayed signal that transitions
  second. Delayed signals are only created for the reference and data
  signals and not for any condition signals associated with them."
  L52557-52565 gives the Figure 31-6 delays dCP 10.01, dD 0.00, dTI 20.02,
  dTE 2.02 (a unit test of the delay solver reproduces them).
- SV 31.9.3 L52573-52577 (V 15.5.3 L17336): "Notifier variables in negative
  timing checks shall be toggled when the timing check detects a timing
  violation, which occurs when the delayed signals as measured by the
  adjusted timing check values are in violation".
- SV 31.9.4 L52580-52584 (V 15.5.4 L17344-17350): "the delayed reference
  and data signals become copies of the original reference and data signals.
  The same occurs if an invocation option turning off all timing checks is
  used."
- `$timeskew`, SV 31.4.2 L51506-51522 (V 15.3.2 L16400): "(timecheck time) -
  (timestamp time) > limit ... $timeskew shall also not report a violation if
  a new timestamp event occurs exactly at the expiration of the time limit.
  The default behavior for $timeskew is timer-based. A violation shall be
  reported immediately upon an elapse of time after the reference event equal
  to the limit, and the check shall become dormant ... if a data event occurs
  within the limit, then a violation shall not be reported, and the check
  shall become dormant immediately. This check shall also become dormant if
  it detects a conditioned reference event when its condition is false and
  the remain_active_flag is not set." Cases 1-4 (L51555-51597) are the four
  flag combinations of `skew.v`.
- `$fullskew`, SV 31.4.3 L51655-51700 (V 15.3.3 L16524): "limit is set to
  limit1 when the reference event transitions first and set to limit2 when
  the data event transitions first ... In the timer-based mode, a second
  timestamp event that occurs within the time limit starts a new timing
  window that replaces the first one, unless the second timestamp event has
  an associated condition whose value is false. In such a case, the behavior
  of $fullskew depends on the remain_active_flag."
- `$nochange`, SV 31.4.6 L51934-51960 (V 15.3.6 L16822): "a negative offset
  for start edge shrinks the region by starting the region later ... a
  negative offset for the end edge shrinks the region by ending it earlier
  ... (beginning of time window) = (leading reference edge time) -
  start_edge_offset (end of time window) = (trailing reference edge time) +
  end_edge_offset ... (beginning of time window) < (data event time) < (end
  of time window)".

## Derivation

- A non-negative `$setuphold(r, d, S, H)` is `$setup(d, r, S)` plus
  `$hold(r, d, H)`; `$recrem(r, d, REC, REM)` is `$removal(r, d, REM)` plus
  `$recovery(r, d, REC)`, with the ADV-009 windows (A10-D1).
- Without the option a negative limit counts as 0 (one warning per check) and
  explicit delayed signals are copies (A10-D2). With it, per check W = S + H
  (REM + REC): W < 0 sets the negative limits to 0 with a warning, 0 <= W <= 1
  keeps them with a warning and the check never runs (its window is shorter
  than two units, A10-D4), and the delays are the smallest non-negative
  solution of `d_ref - d_data >= m - S` (for S != 0; `>= -S` for S = 0) and
  `d_data - d_ref >= m - H` (likewise), m = 1 for W >= 2 and 0 otherwise, per
  signal and edge class (rise: 01, 0x, x1 and the z forms; fall: the others);
  a class no check constrains takes the other class's delay and a delayed
  signal starts x (A10-D3, A10-D4). An inconsistent set zeroes the most
  negative limit on the cycle and repeats (A10-D5). Delayed signals are
  transport delays: a transition landing before a pending one of its bit
  cancels it (A10-D10).
- The checks read the delayed signals with the limits adjusted by the delays
  of the compared transitions (`L + d_check - d_stamp`; a nonzero limit
  adjusted to 0 or less counts as 0 with a warning), toggle the notifier at
  the delayed time and report the undelayed times (A10-D6).
- `$nochange` offsets may be negative regardless of the option; a window
  whose end can pass before the delayed trailing edge decides its data events
  at that edge, each in-window event a violation (A10-D7, A10-D8).
- `$timeskew` and `$fullskew` read the undelayed signals; their flags are
  constant expressions, nonzero sets them (A10-D9).

## Fixtures

- `combined.v` (no option needed): the reference rises at 100, the data at
  100 + OFF (both at 100: data first `0d` or reference first `0r`).
  `$setuphold(..., 10, 5)` violates for data in (90, 105) and at 100 in
  either order; `(10, 0)` only for (90, 100); `(0, 5)` for [100, 105);
  `$recrem(posedge r, posedge d, 10, 5)` for (95, 100), at 100 and in
  [100, 110). Each violation is reported at the later event.
- `negative.v`: `na` = `(-3, 8)`, W = 5: dr = 4, dd = 0, window (103, 108):
  +4 and +7 violate at 104 and 107; +3 and +8 are the end points. `nb` =
  `(8, -3)`: dd = 4, window (92, 97): -7 and -4 violate at the delayed
  reference (100). `nc` = `(-5, 7)` without delayed names: implicit delays,
  window (105, 107): only +6. `nd` = `$recrem(.., 8, -3)` (REM -3, REC 8):
  window (103, 108). `ne` = `(-5, 4)`: W < 0, the setup becomes 0 (warning),
  the hold window [100, 104). `nf` = `(-5, 5)`: W = 0, warning, no
  violation. Without the option every negative limit is 0: `na` violates
  for +2..+7, `nb` for -7..-2, `nc` for +5 and +6, `nd` for +3..+7 and `nf`
  keeps its hold window [100, 105). The traces print the delayed signals:
  - `t` (`na`): dr follows r by 4 for both edges (the falling class is
    unconstrained and takes the rising delay); dd follows d undelayed,
    including a one-tick pulse.
  - `u` (`nb`): dd follows d by 4, including a one-tick pulse and x.
  - `v` (`pe`, the 31.9.1 example): the rising check needs dC - dD >= 4,
    the falling one dC - dD >= 8 (with the one-tick margin of A10-D3), so dC
    is 8 and C rising at 170 shows dC at 178.
  - `w` (`pg`: `(8, -5)` for rising and `(8, -1)` for falling data): dD
    rises 6 and falls 2 after D. The pulse rising at 200 and falling at 201
    is cancelled (the fall lands at 203, before the pending rise at 206); the
    rise at 210 lands at 216 and the fall at 220 at 222.
  Without the option the delayed signals are copies (`negative_off.out`),
  and with `--no-timing-checks` they are copies and no notifier changes
  (`negative_disabled.out`).
- `conditions.v` (`sc`: `$setuphold(posedge r, d, 10, 5, n, ts, tc)`, `se`:
  the same with `(ts == 1'b1)` and `(tc === 1'b1)`, `sa`: `posedge r &&& en`,
  `rc`: `$recrem(posedge r, posedge d, 10, 5, n, ts, tc)`, `sn`: `(-3, 8)`
  with only a timestamp condition). The setup (data at 95, reference 100)
  gates the data (stamp) with `ts` and the reference with `tc`; the hold
  (reference 100, data 102) gates the reference with `ts` and the data with
  `tc`. c1 violates; c2 (ts 0) and c3 (tc 0) do not; c4..c6 the same for the
  hold. c7: ts falls after the stamp, c8: tc rises before the timecheck: both
  violate (read at each event). c9: `==` with x enables; c10: `===` with x
  does not. c11/c12: the `&&&` condition false: none; c13 true: violation.
  c14..c16: `$recrem` removal (data 97) and recovery (data 103). c17/c18:
  with the option the reference is delayed to 104, where c17's `ts` is 1 and
  c18's is 0; without it the reference is read at 100 with the opposite
  values.
- `skew.v`: `i1*`/`i2*` follow Figure 31-1 to 31-3 cases 1-4 for
  `$timeskew(posedge CP &&& MODE, negedge CPN, 50, ...)` and
  `$fullskew(..., 50, 70, ...)`. Timer-based checks report at the expiry
  time (150 for a reference at 100) and turn dormant; event-based ones
  report at the late data event and, with remain_active, at each later one
  (i1d: 7 changes up to 240). A reference with MODE false turns the check
  dormant unless remain_active is set (i2*). `b1`: data exactly at the limit
  (150) is no violation; `b2` at 151 is reported at the expiry 150; `b3`
  and `b4`: a new reference at or within the expiry restarts the window and
  the later data is in time. Limit 0: `b5` data in the reference's time step
  is no violation; one tick late it is reported at 100 by the timer (`b7`)
  and at 101 by the event-based check (`b6`). `f*`: `$fullskew` limits 50
  after a reference and 70 after data; `f3`: data at 200 then the reference
  at 270 (= limit 2): none; `f4`: simultaneous: none.
- `nochange.v`: `$nochange(negedge c, d, -2, -3)` with c low 100..120 has the
  window (102, 117), decided at the trailing edge: a0 (data 105, 110, 118)
  and a1 (103, 117) violate at 120, a2 (101) not. `(2, -3)`: (98, 117): b0
  (97, 99), b1 (116, 117), b2 (99) at 120; b3 (98) not. `(-2, 3)`: (102,
  123), decided at each event: e1 at 103, e2 twice (121, 122); e0 (101, 102)
  and e3 (123) not. Notifier changes are counted once per time step. `ng`:
  `$nochange(posedge C, E, 0, 0)` on C delayed by 6 by a `(-5, 10)` check:
  window (200, 220) in undelayed times decided at the delayed trailing edge
  226 (g0 at 203, g2 at 219; g1 at 223 not); without the option at the
  events themselves (203 and 219).
- `inconsistent.v`: `$setuphold(posedge A, B, -5, 10, , , , dA, dB)` and
  `$setuphold(posedge B, A, -6, 10, , , , dB, dA)` need dA > dB and dB > dA.
  The warning zeroes -6, then -5; no delays remain and the second check's
  hold window [100, 110) catches B at 106.
- `adjusted_limit.v`: a `$hold(posedge r, d, 2)` beside a `(-3, 8)` check
  that delays r by 4: its limit adjusts to 2 + 0 - 4 = -2, counts as 0 with
  a warning, and the data change with the undelayed reference at 100 is no
  violation.
- Negative cases (`neg_*.v`) stop before the model runs with the location of
  the offending argument: a delayed signal of another width, a delayed signal
  naming its own signal, two delayed names for one signal, one delayed name
  for two signals and an unknown (`x`) `$timeskew` flag.
