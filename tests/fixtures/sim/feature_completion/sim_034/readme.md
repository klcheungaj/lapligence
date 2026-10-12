# SIM-034 synchronous drives, cycle delays and virtual clocking outputs

IEEE 1800-2009 §§4.4-4.5, 14.11-14.16 and 25.9 supply the oracles. Every
positive source runs through the public CLI in both optimizer modes on the
legacy and compact (portable and GMP) value backends; `drive_timeline` and
`vif_drives` also run after the frontend snapshot and owned Db are destroyed,
at model `-O0` and `-O3`. All `.out` files are derived by hand from the
clauses and the timelines below, never captured from llg.

## Clauses

Line numbers refer to the `pdftotext -layout` extraction
`SystemVerilog-1800-2009.txt`.

- 14.16, L20064-20067: "For zero skew clocking block outputs with no cycle
  delay, synchronous drives shall schedule new values in the Re-NBA region of
  the time step corresponding to the clocking event. For clocking block
  outputs with non-zero skew, or drives with non-zero cycle delay, the
  corresponding signal shall be scheduled to change value in the Re-NBA region
  of a future time step."
- 14.16, L20069-20074: "For each clocking block output whose target is a net,
  a driver on that net shall be created. The driver so created shall have
  (strong1, strong0) drive strength [...] The created driver shall be
  initialized to 'z, hence, the driver has no influence on its target net
  until a synchronous drive is performed to the corresponding clockvar."
- 14.16, L20087-20092: "clockvar_expression ::= clockvar select" and "The
  clockvar_expression is a bit-select, slice, or the entire clocking block
  output whose corresponding signal is to be driven (concatenation is not
  allowed)".
- 14.16, L20103-20108: "Like a nonblocking intra-assignment delay, it shall
  not cause execution of the statement to block. The right-hand side
  expression shall be evaluated immediately even when a cycle_delay is
  present. However, updating of the target signal shall be postponed for the
  specified number of cycles of the target clockvar's clocking block, plus any
  clocking output skew specified for that clockvar."
- 14.16, L20110: "No other form of intra-assignment delay syntax shall be
  legal in a synchronous drive to a clockvar."
- 14.16, L20144-20147: "Such drive statements shall execute without blocking,
  but shall perform their drive action as if they had executed at the time of
  the next clocking event. The expression on the right-hand side of the drive
  statement shall be evaluated immediately, but the processing of the drive is
  delayed until the time of the next clocking event."
- 14.16, L20159-20161: "It shall be an error to write to a clockvar except by
  using the synchronous drive syntax described in this subclause."
- 14.16.1, L20167-20168: "a drive does not change the clocking block input.
  This is because reading the input always yields the last sampled value, and
  not the driven value."
- 14.16.2, L20194-20195: "When more than one synchronous drive on the same
  clocking block output (or inout) is scheduled to mature in the same Re-NBA
  region of the same time step, the last value is the only value driven onto
  the output signal."
- 14.16.2, L20234-20235 and L20244-20260: drives "scheduled to mature at
  different future times due to the use of cycle delay [...] shall each mature
  in their corresponding future cycles"; in the example `#1 cb.v <= ##2
  expr3;` issued after cycle 1 "Matures in cycle 3" and `##1 cb.v <= ##1
  expr4;` "Matures in cycle 3, v is assigned expr4".
- 14.11, L19810-19812: "If no default clocking has been specified for the
  current module, interface, checker, or program, then the compiler shall
  issue an error."
- 14.11, L19824-19828: "Cycle delays of ##0 are treated specially. If a
  clocking event has not yet occurred in the current time step, a ##0 cycle
  delay shall suspend the calling process until the clocking event occurs.
  When a process executes a ##0 cycle delay and the associated clocking event
  has already occurred in the current time step, the process shall continue
  execution without suspension. When used on the right-hand side of a
  synchronous drive, a ##0 cycle delay shall have no effect, as if it were not
  present."
- 14.11, L19830-19831: "Cycle delay timing controls shall not be legal for use
  in intra-assignment delays in either blocking or non-blocking assignment
  statements."
- 14.13, L19909-19912 and L19921-19923: a non-`#0` input skew samples the
  Postponed value before the event ("#1step" is the default); the clocking
  block "shall update its sampled values before triggering the event
  associated with the clocking block name. This event shall be triggered in
  the Observed region."
- 4.5, L3386-3392: the inner loop runs Active through Post-Observed ("while
  (any region in [Active ... Post-Observed] is nonempty) { execute_region
  (Active); [...] move events in R to the Active region; }") before the
  reactive set; 24.3.1, L43248-43249: program statements sensitive to design
  signals "are scheduled in the Reactive region".
- 25.9, L44891-44895: "A single virtual interface variable can thus represent
  different interface instances at different times throughout the simulation.
  [...] Attempting to use a null virtual interface shall result in a fatal
  run-time error."

## Positives

### `drive_timeline` (A02)

Posedges fall at 10, 20, 25, 40 and 50. `d` starts at 00 and `always
@(posedge clk) d <= d + 1` makes it 01, 02, 03, ... in the NBA region of
each edge, so the `#1step` sample `cb.d` is 00 at 10 and 01 at 20. Monitor
`M` prints after every change of `o p q s a b` (one line per wake, after the
Re-NBA region that changed them).

- 10, after `@(cb)` (Observed, then Active): `P cb.d=00 d=01 o=00`. The drives
  issued now are on-event: `o <= cb.d + 50` (50), `a <= A1`, `b <= cb.a` (the
  sampled `a`, 0A, 14.16.1) commit in the Re-NBA region of 10; `p <= 11`
  (`#2` skew) in that of 12; `q <= ##2 22` matures at the second following
  event (25). The `#0` line runs in Inactive before Re-NBA (old values). The
  program prints in Reactive, still before Re-NBA (`R o=00 a=0a b=00`, 4.5 and
  24.3.1). Re-NBA commits, `M` prints the new values, `$strobe` prints them in
  Postponed.
- 12: `p=11`.
- 13 (off-event): `q <= 33` matures at 20; `q <= ##2 44` at the second
  following event (25); `s[3:0] <= 7` and `s[7:4] <= ##1 8` at 20; `p <= 12`
  at 20 plus the `#2` skew (22). `##1` resumes after the cb event at 20.
- 20: `P ##1 cb.d=01`; `##0` does not suspend (the event occurred); `q <= ##0
  55` is the same as `q <= 55` (14.11) and is issued after the edge matured
  `q <= 33`, so the Re-NBA region drives 33 then 55 and only 55 is visible:
  `M ... q=55 s=87`.
- 22: `p=12`. The off-event `o <= 60` matures at 25; `##0` waits for that
  event.
- 25: `P ##0 waited`; `q <= ##2 22` (issued at 10) and `q <= ##2 44` (13)
  mature in the same Re-NBA region and the later-issued 44 is driven, with
  `o=60`. `p <= ##1 77` matures at 40 (+2: 42); `o <= ##(n) 99` takes `n = 1`
  at issue (40) even though `n` becomes 5 at once; `##(n - 3)` waits 2 events.
- 40: `o=99`; 42: `p=77`; 50: `P ##2 done`; `$finish` at 53.

### `vif_drives` (A01)

`sync_bus` has a variable output `b` with `#1` skew, a wire inout `c` with a
second continuous driver `c_drv`, and a wire output `n` driven only by the
clocking block. `b1` is clocked by `c1` (posedges 3, 9, 15, 21, 27, 33), `b2`
by `c2` (posedges 5, 7, 16, 30). At 0, `b2.c_drv = 0F`. At 1 (off-event for
both clocks) with `v` bound to `b1`: `b <= 11` (edge 3, +1: 4), `n <= 3`
(edge 3), `c <= ##2 C1` (edges 3, 9: 9). Then `v` is rebound to `b2`; the
queued drives stay on `b1`. Through `v` (now `b2`): `b <= ##2 22` (edges 5, 7,
+1: 8), `c <= F0` (edge 5; resolves with `c_drv = 0F` to `xx`), `n[1:0] <= 10`
(edge 5; the other bits of the clocking driver stay `z`: `zz10`). A class
object holding a handle to `b1` issues `b <= ##1 33` (edge 3, +1: 4), after the
`11` drive, so 33 is driven. At 10, `b2.c_drv = zz` leaves only the clocking
driver: `c = f0`. `v` is rebound to `b1`; after `@(b1.sb)` at 15 the sampled
`c` is `c1`. At 17 the array elements `vs[1]` (`b2`) and `vs[0]` (`b1`) drive
`b`: `44` at the `c2` edge 30 + 1 and `55` at the `c1` edge 21 + 1. The monitors
print every change after time 0 (the time-0 assignment is not printed).

### `vif_selects`

`m` is `logic [1:0][3:0]`, `asc` is `logic [0:7]`. Through `v` bound to `bj`:
`m[1][2] <= 1`, `m[0] <= 5`, `m[1][1:0] <= 11` give `m = 0111_0101`;
`asc[0:3] <= A` and `asc[6] <= 1` give `1010_0010`; `v8[k+:4] <= C` with
`k = 4` at issue gives `c0`. Through the modport view `w` bound to `bi`:
`v8 <= ##1 3C` (the first edge, at 1) and `asc[7] <= 1` give `bi.asc =
0000_0001`, `bi.v8 = 3c`. Rebinding `v` and changing `k` after issue changes
nothing.

### `inout_net` (A03)

`w` has the continuous driver `other` and the clocking block's own driver.
At 1 `other = 0001` (the clocking driver is still `z`): `w = 0001`. The drive
at 5 makes the clocking driver `0011`: bit 1 has 0 and 1 at strong strength,
`w = 00x1`. At 15 the sample (Postponed of 14) is `00x1`; `other = zzzz`
leaves `0011`. At 25 the drive `zz10` releases the upper bits: `w = zz10`.

### `virtual_clocking_witness`

FND-002 witness `virtual_clocking_drive` (L-F12-02-03, L-F12-04-05), source
unchanged: the drive issued at 0 through `v` matures at the edge at 1, so
`i.a` is 7 at 2.

## Negatives

| Fixture | Clause | Diagnostic |
| --- | --- | --- |
| `neg_intra_delay_drive` | 14.16 L20110 | clocking signals cannot be driven with a timing control other than a cycle delay |
| `neg_blocking_drive` | 14.16 L20159-20161 | can only be written via a synchronous drive |
| `neg_cycle_plain_nba` | 14.11 L19830-19831 | intra-assignment cycle delays can only be used with clocking signals |
| `neg_cycle_prefix_no_default` | 14.11 L19810-19812 | no default clocking has been specified |
| `neg_output_dynamic_skew` | 14.4 L19518 ("A skew shall be a constant expression") | reference to non-constant variable |
| `neg_vif_input_drive` | 14.16 L20050 (outputs and inouts drive) | cannot write to input clocking signal |
| `neg_vif_compound_drive` | 14.16 L20159-20161 | can only be written via a synchronous drive |
| `neg_vif_concat_drive` | 14.16 L20091-20092 | cannot be part of a concatenation |

The SIM-033 negatives `neg_compound_drive`, `neg_concat_drive`,
`neg_cycle_no_default` and `neg_dynamic_skew` cover the same rules on
concrete clocking blocks.
