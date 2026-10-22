# ADV-007 specify blocks, specparams and simple module paths

Every expected output is hand-derived from the clauses quoted below; none is
produced by llg. The Verilog (`.v`) sources run in both editions
(`--edition v2001` and `sv2009`), and every positive source runs in both
optimizer modes on the legacy, compact/portable and compact/GMP value backends
(`tests/sim_feature_completion/adv_007.rs`). The portable decisions are the
`A7-D*` cases in `tests/fixtures/sim/lrm_decisions/` (see
`docs/lrm_decisions.md`).

## Clauses

Line numbers are those of the `pdftotext -layout` extractions
`Verilog-1364-2001.txt` (V) and `SystemVerilog-1800-2009.txt` (SV).

- Specparams, V 3.11.3 L3409-3418 (SV 6.20.5 L6785-6796): "The keyword
  specparam declares a special type of parameter which is intended only for
  providing timing and delay values, but can appear in any expression that is
  not assigned to a parameter and is not part of the range specification of a
  declaration. ... A specify parameter declared outside a specify block shall
  be declared before it is referenced. The value assigned to a specify
  parameter can be any constant expression. A specify parameter can be used
  as part of a constant expression for a subsequent specify parameter
  declaration." Table 8 L3430 (SV Table 6-11 L6818): parameters "May not be
  used inside specify blocks".
- Path restrictions, V 14.2.1 L14107-14114: "The module path source shall be
  a net that is connected to a module input port or inout port. — The module
  path destination shall be a net or variable that is connected to a module
  output port or inout port. — The module path destination shall have only one
  driver inside the module." SV 30.4.1 L49408-49413 keeps the first two rules
  and drops the third; V 14.5 L15021: "Module path output nets shall not have
  more than one driver within the module."
- Connections, V 14.2.5 L14518-14535 (SV 30.4.5 L49780-49797): "In a full
  connection, every bit in the source shall connect to every bit in the
  destination. ... In a parallel connection, each bit in the source shall
  connect to one corresponding bit in the destination. Parallel module paths
  can be created only between sources and destinations that contain the same
  number of bits."
- Delay values, V 14.3.1 L14766-14770 (SV 30.5.1 L50002-50007): "Each path
  delay expression may be a single value—representing the typical delay—or a
  colon-separated list of three values—representing a minimum, typical, and
  maximum delay, in that order. If the path delay expression results in a
  negative value, it shall be treated as zero." Table 47 L14776 (SV Table 30-2
  L50022-50057): one value `t` for every 0/1/z transition; two values give
  0->1, 0->z, z->1 `trise` and 1->0, 1->z, z->0 `tfall`; three values give
  `trise`, `tfall` and `tz` for 0->z and 1->z; six values are t01, t10, t0z,
  tz1, t1z, tz0; twelve values add t0x, tx1, t1x, tx0, txz, tzx.
- X transitions, V 14.3.2 L14846-14849 (SV 30.5.2 L50102-50105):
  "Transitions from a known state to x shall occur as quickly as possible ...
  Transitions from x to a known state shall take as long as possible". Table
  48 L14871-14893 (SV Table 30-3 L50112-50134): "0 -> x minimum (0 -> z delay,
  0 -> 1 delay); 1 -> x minimum (1 -> z delay, 1 -> 0 delay); z -> x minimum
  (z -> 1 delay, z -> 0 delay); x -> 0 maximum (z -> 0 delay, 1 -> 0 delay);
  x -> 1 maximum (z -> 1 delay, 0 -> 1 delay); x -> z maximum (1 -> z delay,
  0 -> z delay)".
- Delay selection, V 14.3.3 L14915-14922 (SV 30.5.3 L50166-50173): "Active
  specify paths are those whose input has transitioned most recently in time,
  and which have either no condition or whose conditions are true. ... a delay
  must be selected from among them. This is done by comparing the correct
  delay for the specific transition being scheduled from each specify path,
  and choosing the smallest."
- Mixing with distributed delays, V 14.4 L14961-14962 (SV 30.6 L50201-50202):
  "If a module contains module path delays and distributed delays (delays on
  primitive instances within the module), the larger of the two delays for
  each path shall be used."
- Pulses, V 14.6 L15105, L15128 (SV 30.7 L50256-50258, L50269-50270): "Two
  consecutive scheduled transitions closer together in time than the module
  path delay are deemed a pulse. By default, pulses on a module path output
  are rejected. ... By default, both the error limit and the reject limit are
  set equal to the delay."
- Net initial value, V 3.6 L2631 (SV 6.7 L5199-5200): "The default
  initialization value for a net shall be the value z. Nets with drivers shall
  assume the output value of their drivers." A driven path destination
  starts x until its first delayed transition matures (decision A7-D6, as for
  net delays, S2-D10), so every first transition below is from x.

## Derivations

Delays below are written in the twelve-transition order 01, 10, 0z, z1, 1z,
z0, 0x, x1, 1x, x0, xz, zx. A transition caused at time `t` by an input that
last changed at `s` passes the path at `max(t, s + path delay)`; a net delay
on the destination then delays that result like any net delay (A7-D1).

- `transitions.v`: `(3, 5)` expands to 3 5 3 3 5 5 and, by Table 30-3, 0x =
  min(3, 3) = 3, x1 = max(3, 3) = 3, 1x = min(5, 5) = 5, x0 = 5, xz =
  max(5, 3) = 5, zx = min(3, 5) = 3. `(1, 2, 3)` is 1 2 3 1 3 2 with 0x =
  min(3, 1) = 1, x1 = max(1, 1) = 1, 1x = min(3, 2) = 2, x0 = max(2, 2) = 2,
  xz = 3, zx = min(1, 2) = 1. `(1, ..., 6)` gives 0x = min(3, 1) = 1, x1 =
  max(4, 1) = 4, 1x = min(5, 2) = 2, x0 = max(6, 2) = 6, xz = max(5, 3) = 5,
  zx = min(4, 6) = 4 (the example of Table 30-3 gives the same shape). Each
  input change is 100 units after the previous one, so every output changes
  at the change time plus the delay of its own transition; the first one is
  x->0 at time 0 (2, 4, 5, 6 and 10; the twelfth form's x0 is its tenth
  delay). `p12` uses `*>`, which for scalars is the
  same connection.
- `connections.v`: `Y = A | B` with `(A => Y) = (6, 9)` and `(B => Y) =
  (5, 11)` (the example of 30.5.3). Y's first x->0 has both inputs changed at
  0: min(9, 11) = 9. B rises at 100: 105; B falls at 200: 211; A rises at 300:
  306; A falls at 400: 409; both rise at 500: min(6, 5) = 5, 505; both fall at
  600: min(9, 11), 609; A rises at 700: 706; B rises at 800 and A falls at 900
  without changing Y; B falls at 1000 and is the latest input: 1011. In `bus`,
  `p` follows `a` bit by bit with rise 2 and fall 3 (1102; 1202 and 1203 for
  the bits rising and falling at 1200; 1403). `f[1] = ^a` has only the full
  path from `a[3:2]` (5); `f[0] = c` has that path and `c *> f[0]` (1): at 0
  both count as changed (A7-D3), min(5, 1) = 1; at 1300 `c` is the latest
  input, 1301; at 1400 `a[3]` and `c` change together, so `f[0]` falls after
  min(5, 1) = 1 (1401) and `f[1]` rises after 5 (1405). `a[3:2]` excludes
  `a[0]`: the change of `a[0]` at 1200 (bits 0 and 2 of `v` change from 0101
  to 1100) does not change `f`.
- `duplicate_paths.v`: a path declared twice and a full path over bits of an
  earlier parallel path are further active paths between the same bits; the
  example 2 of SV 30.5.3 (L50184-50196) declares `A => Y` five times. In
  `overlap`, `o[0]` has `a => y` (3) and `a[0] *> y[0]` (1): min(3, 1) = 1;
  `o[1]` has only 3. In `twice`, `t` has `(1, 4)` and `(3, 2)`: rise
  min(1, 3) = 1, fall min(4, 2) = 2. x->0 at 0 (falls): `o[0]` 1, `t` 2,
  `o[1]` 3; rise at 10: 11, 11, 13; fall at 20: 21, 22, 23 (decision A7-D4).
- `distributed.v`: `fig3` (gates 0 and 1, path 22, Figure 30-3): Q follows D
  at 22, 122, 222. `fig4` (gates 10 and 20, path 22, Figure 30-4): Q follows D
  after the distributed 30: 30, 130, 230; its gate output starts X (decision
  S2-D4), which reaches Q through the path at 0 + 22 = 22. `netd` (net delay
  4, path 2): 2 + 4, so 6, 106, 206. `netp` (net delay 1, path 6): 6 + 1, so
  7, 107, 207. `hier`
  (an inverter instance drives Q, path 4): 4, 104, 204. `io` (an inout
  destination driven only inside, path 3): 3, 103, 203, and the outside copy
  `qr` follows at the same times.
- `specparams.v` (`timescale 1ns/100ps`): `body_sp` delays (3, 3 + 1): z->0
  at 4.0, rise at 13.0, fall at 24.0. `block_sp` delays (1.5, 2:5:9 -> 5):
  5.0, 11.5, 25.0; its module-body `$display` reads the specify-block
  specparams. `neg_sp` delays (-2 -> 0, 3): 3.0, 10.0, 23.0. `mtm` delays
  (1:2:3 -> 2, 4:6:8 -> 6): 6.0, 12.0, 26.0.
- `pulses.v` (rise 3, fall 8): x->0 at 8; 100/105 gives 103/113; the 2-unit
  pulse at 200 is shorter than the rise delay and is rejected (the new value
  equals the output, so the pending update is descheduled); 300/350 gives
  303/358.
- `force_release.v`: y, v (net delay (2, 3)) and s (scalar net delay (2, 3))
  first change x->0 after the fall delay 3. The rise of a to 11 at 10 would
  reach them at 12, but the force at 11 publishes 01 / 0 at once and drops the
  pending updates; the release at 21 publishes the driven 11 / 1 at once (SV
  10.6.2 L13397-13399: "A force procedural statement on a net shall override
  all drivers of the net—gate outputs, module outputs, and continuous
  assignments—until a release procedural statement is executed on the net.
  When released, the net shall immediately be assigned the value determined by
  the drivers of the net."; V 9.3.2 L8928-8929 has the first sentence). The fall
  of bit 0 at 31 then starts from the released 1 and arrives at 34, and the
  fall of bit 1 and of s at 41 arrives at 44.
- `wired.v` (SystemVerilog only): two in-module drivers resolve first. y
  starts x and reaches the resolved z after a's 2 (both inputs count as
  changed at 0, A7-D3). a rises at 10, y z->1 after a's 2: 12; b rises at 20, y 1->x after b's 5 (the 1->x
  delay of `5` is 5): 25; a falls at 30, y x->0 after 2: 32; b falls at 40,
  y 0->z after 5: 45. IEEE 1364-2001 rejects the module (14.2.1).

## Negative cases

- Frontend errors, located at the offending token:
  `neg_parameter_in_specify.v` (A7-D5; the
  frontend warns, llg promotes), `neg_specparam_in_parameter.v`,
  `neg_specparam_in_range.v`, `neg_specparam_before_declaration.v`,
  `neg_source_not_port.v`, `neg_destination_input.v`,
  `neg_parallel_width.v`, `neg_delay_count.v`.
- Later tasks, rejected with their location (ADV-009/ADV-010):
  `neg_timing_check.v`. State-dependent and edge-sensitive paths, `PATHPULSE$`
  and pulse-style declarations are simulated since ADV-008 (`../adv_008/`).
- Not simulated yet: `neg_variable_destination.v` (a legal variable
  destination), `neg_outside_driver.v` (an inout destination also driven
  outside the module), `neg_multidimensional_select.sv` (a select of a
  multidimensional packed terminal).
