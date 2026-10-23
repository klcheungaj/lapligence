# ADV-012 SDF annotation (`$sdf_annotate`, `--sdf`)

llg reads every SDF file a design names when it generates the model
(ADV-011 grammar and resolution), values it for each call (member
selection, scaling, rounding, pulse limits) and applies the valued file when
the call executes, or before simulation starts for `--sdf` files and calls
that lead an initial procedure (decision A12-D1). The tests are in
`tests/sim_feature_completion/adv_012.rs`; positive cases run in both
optimizer modes on every value backend. The SDF files and the cell library
were written for these tests, and every expected timestamp, value, notifier
and warning below was derived by hand from the quoted clauses before the
fixture was run. The decisions are registered in
[`docs/lrm_decisions.md`](../../../../../docs/lrm_decisions.md#sdf-back-annotation-adv-012)
with portable cases `A12-D*` in `tests/fixtures/sim/lrm_decisions/`.

## Files

- `paths.v`, `paths.sdf`, `inc.sdf`, `paths.out`: IOPATH delays, delval pulse
  limits, PATHPULSE, PATHPULSEPERCENT and INCREMENT files applied while
  simulating.
- `checks.v`, `checks.sdf`, `checks.out`: every Table 32-2 entry on one cell
  per check; the test also requires each violation report.
- `negative.v`, `negative.sdf`, `negative.out` (`--neg-timing-checks`):
  annotated negative `$setuphold` limits against the same limits declared.
- `mtm.v`, `mtm.sdf`, `partial.sdf`, `mtm.cfg`, `mtm.out`, `mtm_min.out`
  (`--delay-select min`): mtm_spec, scale_factors, scale_type, a
  configuration file and the arguments that override it, rounding, a
  missing triple member and the log file.
- `device.v`, `device.sdf`, `tri.sdf`, `dinc.sdf`, `turnoff.cfg`,
  `device.out`: DEVICE on primitives, a UDP, three-state gates, paths, one
  output and one named primitive, then an INCREMENT on a gate.
- `interconnect.v`, `interconnect.sdf`, `netdelay.sdf`, `ms.sdf`, `min.cfg`,
  `average.cfg`, `last.cfg`, `interconnect.out`: PORT, INTERCONNECT,
  INTERCONNECT_MIPD policies and NETDELAY to ports and nets.
- `startup.v`, `startup.sdf`, `cell.sdf`, `startup.out`: `--sdf`.
- `diagnostics.v`, `malformed.sdf`, `unmatched.sdf`, `values.sdf`,
  `override.sdf`, `unknown.cfg`, `diagnostics.out`: diagnostics; the test
  requires the exact warnings.

## Clauses

Line numbers are those of the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt` (SV).

- SV 32.3 L52627-52628: "It shall issue a warning for any data it is unable
  to annotate." L52635-52636: "Any SystemVerilog timing value for which the
  SDF file does not provide a value shall not be modified during the
  backannotation process, and its prebackannotation value shall be
  unchanged."
- SV 32.4 L52644-52646: "Backannotation into SystemVerilog is done by
  matching SDF constructs to the corresponding SystemVerilog declarations and
  then replacing the existing SystemVerilog timing values with those from the
  SDF file."
- SV Table 32-1 L52686-52692: "(DEVICE... All specify paths to module
  outputs. If no specify paths, all primitives driving module outputs.
  (DEVICE port_instance... If port_instance is a module instance, all specify
  paths to module outputs. If no specify paths, all primitives driving module
  outputs. If port_instance is a module instance output, all specify paths to
  that module output. If no specify path, all primitives driving that module
  output."
- SV Table 32-2 L52752-52772: "(SETUP v1... $setup(v1), $setuphold(v1,x)
  (HOLD v1... $hold(v1), $setuphold(x,v1) (SETUPHOLD v1 v2... $setup(v1),
  $hold(v2), $setuphold(v1,v2) (RECOVERY v1... $recovery(v1), $recrem(v1,x)
  (REMOVAL v1... $removal(v1), $recrem(x,v1) (RECREM v1 v2...
  $recovery(v1), $removal(v2), $recrem(v1,v2) (SKEW v1... $skew(v1),
  $timeskew(v1) ... (WIDTH v1... $width(v1,x) (PERIOD v1... $period(v1)
  (NOCHANGE v1 v2... $nochange(v1,v2)".
- SV 32.4.4 L52906-52907: "When annotating a PORT construct, the SDF
  annotator shall search for the port and, if it exists, shall annotate an
  interconnect delay to that port that shall represent the delay from all
  sources on the net to that port." L52909-52913: "When annotating a
  NETDELAY construct, the SDF annotator shall check to see if it is
  annotating to a port or a net. If it is a port, then the SDF annotator
  shall annotate an interconnect delay to that port. If it is a net, then it
  shall annotate an interconnect delay to all load ports connected to that
  net." L52917-52919: "If the source port is not found or if the source port
  and the load port are not actually on the same net, then a warning is
  issued, but the delay to the load port is annotated anyway."
- SV 32.5 L52970-52971: "SDF annotation is an ordered process. The
  constructs from the SDF file are annotated in their order of occurrence."
- SV 32.6 L53025-53027: "Each call to the $sdf_annotate task annotates the
  design with timing information from an SDF file. Annotated values either
  modify (INCREMENT) or overwrite (ABSOLUTE) values from earlier SDF files."
- SV 32.7 L53033-53037: "the default values annotated for pulse limits shall
  be calculated using the percentage settings for the reject and error
  limits. By default, these limits are 100%"; L53050-53051: "Annotations in
  INCREMENT mode can result in pulse limits less than 0, in which case they
  shall be adjusted to 0."; L53069-53071 (after the page break): "There are
  two SDF constructs that annotate only to pulse limits, PATHPULSE and
  PATHPULSEPERCENT. They do not affect the delay. When PATHPULSE sets the
  pulse limits to values greater than the delay, SystemVerilog shall exhibit
  the same behavior as if the pulse limits had been set equal to the delay."
- SV 32.8 L53119-53121: "For other delays that can have at most three
  values, the expansion of less than three SDF delays into three
  SystemVerilog delays is covered in Table 28-9. More than three SDF delays
  are reduced to three SystemVerilog delays by simply ignoring the extra
  delays."
- SV 28.16 L48374-48376: "When two delays are given, the first delay shall
  specify the rise delay, and the second delay shall specify the fall delay.
  The delay when the signal changes to high impedance or to unknown shall be
  the lesser of the two delay values."
- SV 32.9 L53153-53156: "If the module_instance is not specified, the SDF
  annotator uses the module containing the call to the $sdf_annotate system
  task as the module_instance for annotation."; L53161-53162: "log_file ...
  Each individual annotation of timing data from the SDF file results in an
  entry in the log file."; L53164-53166: "mtm_spec—is an optional character
  string argument specifying which member of the min/typ/max triples shall be
  annotated ... This overrides any MTM_SPEC keywords in the configuration
  file."; Table 32-5 L53172-53177: "MAXIMUM Annotates the maximum value
  MINIMUM Annotates the minimum value TOOL_CONTROL (default) Annotates the
  value as selected by the simulator TYPICAL Annotates the typical value";
  L53180-53183: "scale_factors ... "1.6:1.4:1.2" causes minimum values to be
  multiplied by 1.6, typical values by 1.4, and maximum values by 1.2. The
  default values are 1.0:1.0:1.0. The scale_factors argument overrides any
  SCALE_FACTORS keywords in the configuration file."; Table 32-6
  L53203-53209: "FROM_MAXIMUM Applies scale factors to maximum value
  FROM_MINIMUM Applies scale factors to minimum value FROM_MTM (default)
  Applies scale factors to min/typ/max values FROM_TYPICAL Applies scale
  factors to typical value".
- SV 31.9.1 L52377-52379: "A timing violation shall be triggered if the
  signal changes in the violation window, exclusive of the end points.
  Violation windows smaller than two units of simulation precision cannot
  yield timing violations."

## Derivation

Common rules: a value in SDF TIMESCALE units is multiplied by the scale
factor of the selected member and rounded once, half away from zero, to the
precision of the module it lands in (1 ps here). One delay value gives every
transition, two give rise and fall (Table 32-4, Table 28-9 for gates, the
turn-off the smaller). A plain delval derives the pulse limits from the new
delay (100%), `((d) (r) (e))` gives them, and PATHPULSE limits above the
delay count as the delay. An annotation applies when its call executes:
events already scheduled keep their times. A path transition that lands
less than the reject limit after the pending one cancels both; one between
the reject and error limits turns the leading edge into x (ADV-008 pulse
rules). Outputs are printed MSB first.

- `paths.v` (cells `BUF` with `(a => y) = (1, 1)`; `y[k]` from `uk`): u0
  (5)(7); u1 (2:3:4)(6:7:8), typical 3/7; u2 delay 10, reject 4, error 8;
  u3 delay 10 then PATHPULSE 4/8; u4 delay 10 then PATHPULSEPERCENT 60/90
  (6/9); u5 delay 10, limits 10. The call precedes any timing control, so
  the time-0 transitions use it: y0 and y1 fall at 7, the others at 10. The
  rise at 20 lands at 23 (u1), 25 (u0) and 30; the fall at 40 at 47 and 50.
  The 2 ns pulse at 100 is shorter than every reject limit: no change. The
  6 ns pulse at 200: u0 and u1 pass (213), u2-u4 give x from 210 to 216 (6
  is between reject and error), u5 rejects it. The 9 ns pulse at 300: u2-u4
  pass (310-319), u5 rejects it. At 400 the first `inc.sdf` (+2 rise, -3
  fall on u0) leaves u0's rise due at 401 in place; then u0 falls after 4
  (424), rises after 7 (447); after the second, 9 and 1 (481, 509); after
  the third, 11 and 0 (540, 571): the fall delay 1 - 3 stops at 0.
- `checks.v`: every cell declares its check with limit 1 and `checks.sdf`
  annotates the Table 32-2 entry that maps onto it. Each phase violates the
  annotated limit once: setup 6 (data 3 before the clock, 103), hold 6 (data
  4 after, 204), SETUPHOLD 6 6 (303 and 307), recovery 7 (clock 5 after the
  reset, 405), removal 8 (reset 5 after the clock, 505), RECREM 7 8 (605 and
  645), skew 3 (data 2 after the clock is within it, 4 after violates at
  724), width 12 (a high phase of 7, 807; 15 does not), period 40 (30 apart,
  930; 45 does not), NOCHANGE 2 3 (data 2 after the trailing edge is inside
  the window extended by 3, 1012). Each notifier toggles at its violation,
  and the report names the annotated limit in picoseconds.
- `negative.v` (`--neg-timing-checks`): `h` declares `(-2, 5)`, `a`
  declares `(1, 1)` and is annotated `(-2, 5)`, `z` is annotated `(-3, 4)`.
  The delayed clock of `h` and `a` is the smallest delay with one precision
  unit of margin (ADV-010 decision A10-D3): 2 ns + 1 ps, so `dclk` rises at
  102.001 and falls at 112.001; `z`'s is 3.001 (103.001, 113.001). Data 4 ns
  after the clock is inside the window (2, 5) of `h` and `a` (violations at
  104, notifiers 1) and at the end point of `z`'s window (3, 4), which is
  exclusive. Data 1 ns before the clock (201) is outside every window and the
  delayed flops capture 0; data 1 ns after the clock (301) is before the
  delayed clock and is captured (q 1 at 302.001 and 303.001).
- `mtm.v` (`mtm.sdf`: `(2:4:6) (3:5:7)`, `y[k]` from `mk`): m0 TOOL_CONTROL
  follows `--delay-select` (typical 4/5 by default, minimum 2/3 in
  `mtm_min.out`); m1 MINIMUM 2/3; m2 MAXIMUM 6/7; m3 TYPICAL scaled by 2:
  8/10; m4 MAXIMUM with FROM_MINIMUM and factor 0.5: 1/1.5; m5 `mtm.cfg`
  (MINIMUM, factors 3:1:1, FROM_MAXIMUM): 18/21; m6 the same file with the
  argument TYPICAL: factor 1, 6/7; m7 TOOL_CONTROL as m0; m8 `minimum`
  scaled by 0.3333: 0.6666 and 0.9999 ns round to 667 and 1000 ps; m9
  `partial.sdf` `(2::6) (3:5:7)` TYPICAL: the missing rise keeps 1, the
  fall becomes 5. m5's time-0 fall (due at 21) and its rise at 28 are a
  7 ns pulse below the reject limit 18, so y5 stays x until 28. The log
  file `mtm.log`, written when the model is generated, holds the header,
  the plan of each call naming it (m7 and m9, in call order) and is read
  back by the design.
- `device.v`: x0 (`and #1`, no paths) DEVICE (2)(3); x1 (`bufif1 #(1,1,1)`)
  `tri.sdf` (3)(4): turn-off min 3; x2 the same with `TURNOFF_DELAY =
  MAXIMUM`: 4; x3 DEVICE (2)(3)(5); x4 (UDP `ud #1`) (4)(6); x5 (`and` with
  two paths of 1) DEVICE (5) on both paths; x6 DEVICE y (6) on the path to
  `y` only (`z` keeps 1); x7 DEVICE g2 (7) on primitive g2 only (g1 keeps
  1). Rises at 20 land at 21 (z6, y7a), 22 (x0, x3), 23 (x1, x2), 24, 25,
  26, 27; `en` falling at 40 turns x1, x2, x3 off at 43, 44, 45, rising at
  60 turns them on with the rise delay (62, 63). At 100 `dinc.sdf` adds 1
  and -5 to x0: rise 3 (113), fall 0 (130).
- `interconnect.v` (cells without delays): PORT l1.a (3): o1 3 after a; the
  INTERCONNECT s2.y -> l2.a (4)(5); the four MS instances join two
  `bufif1` drivers on one net whose load has INTERCONNECT delays 6 (from d1)
  and 2 (from d2), combined by INTERCONNECT_MIPD: MAXIMUM 6 (default), MINIMUM
  2, AVERAGE 4 and LAST 2 (the last annotated), for either driver; NETDELAY
  l3.a (7) (SDF 2.1, a port); NETDELAY n4 (8): both loads of the net; NETDELAY
  n5[1] (9): bit 1 of l5's port and l5b, not bit 0 (undelayed); the
  INTERCONNECT from `nosuch.y` delays l6.a by 1 from every source, with the
  two warnings.
- `startup.v`: `--sdf startup.sdf` (u0, u2 typical 4/5), then `--sdf
  max:tb.u1=cell.sdf` (INCREMENT maximum +6/+7: 7/8), then `--sdf
  min:tb.u2=cell.sdf` (+2/+3: 6/8), all before time 0: the falls at time 0
  land at 5 and 8.
- `diagnostics.v`: `missing.sdf` and `malformed.sdf` (a syntax error after a
  valid CELL for u4) annotate nothing; in `unmatched.sdf` the unknown
  instance, the wrong cell type, the IOPATH from `b`, the HOLD that no check
  matches and the wildcard cell type without instances are warned about
  while the IOPATH of u0 (2) and the SETUP of f0 (4) apply; in `values.sdf`
  u1's `1e999` rise becomes 1 ps (A12-D3), u2's missing typical rise and
  empty fall keep 1, u3's 13 values are not annotated; the invalid mtm_spec,
  scale_factors, scale_type and the configuration statement `PULSE_DELAY`
  each skip their call. So y0 changes 2 after a, y1 rises after 1 ps and
  falls after 3, the others after 1, and d rising 3 before the clock
  violates the annotated setup 4 at 60.
