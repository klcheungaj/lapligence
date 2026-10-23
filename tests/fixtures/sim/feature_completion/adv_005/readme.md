# ADV-005 bidirectional pass-switch networks

`tran`, `tranif0`, `tranif1`, `rtran`, `rtranif0` and `rtranif1`. Every
expected output follows from the clauses quoted below and the decisions
`A5-D1` to `A5-D6` (`docs/lrm_decisions.md`, cases in
`tests/fixtures/sim/lrm_decisions/`). All sources are plain Verilog-2001 and
run in both editions (`--edition v2001` and `sv2009`), in both optimizer modes,
on the legacy, compact/portable and compact/GMP value backends
(`tests/sim_feature_completion/adv_005.rs`). That test file also compares 160
generated random networks over four phases (loops, self loops, parallel and
opposing switches, up to five unknown controls per network) with an
independent oracle that solves every conducting/nonconducting combination of
the unknown switches (4.9.5).

## Clauses

Line numbers are those of the `pdftotext -layout` extractions
`Verilog-1364-2001.txt` (V) and `SystemVerilog-1800-2009.txt` (SV).

- Network solving, SV 4.9.5 L3552-3555 (V 5.6.5 L5324-5327): "Switch
  processing shall consider all the devices in a bidirectional switch-connected
  net before it can determine the appropriate value for any node on the net
  because the inputs and outputs interact. A simulator can do this using a
  relaxation technique. The simulator can process tran at any time. It can
  process a subset of tran-connected events at a particular time, intermingled
  with the execution of other active events."
- Unknown controls, SV 4.9.5 L3557-3570 (V 5.6.5 L5329-5332): "Further
  refinement is required when some transistors have gate value x. A
  conceptually simple technique is to solve the network repeatedly with these
  transistors set to all possible combinations of fully conducting and
  nonconducting transistors. Any node that has a unique logic level in all
  cases has steady-state response equal to this level. All other nodes have
  steady-state response x."
- Ports, SV 4.9.6 L3575-3576: "Bidirectional connections are analogous to an
  always-enabled tran connection between the two nets, but without any
  strength reduction."
- Pass switches, SV 28.8 L47365-47368 (V 7.6 L6451-6453): "The bidirectional
  pass switches shall not delay signals propagating through them. When
  tranif0, tranif1, rtranif0, or rtranif1 devices are turned off, they shall
  block signals; and when they are turned on, they shall pass signals. The
  tran and rtran devices cannot be turned off; they shall always pass
  signals."
- Switch delays, SV 28.8 L47380-47386 (V 7.6 L6455-6461): "The delay
  specifications for tranif1, tranif0, rtranif1, and rtranif0 devices shall
  be zero, one, or two delays. If the specification contains two delays, the
  first delay shall determine the turn-on delay, the second delay shall
  determine the turn-off delay, and the smaller of the two delays shall apply
  to output transitions to x and z. If only one delay is specified, it shall
  specify both the turn-on and the turn-off delays. If there is no delay
  specification, there shall be no turn-on or turn-off delay for the
  bidirectional pass switch. The bidirectional pass switches tran and rtran
  shall not accept delay specification."
- Terminals, SV 28.8 L47393-47394 (V 7.6 L6467-6468): "The bidirectional terminals of all six
  devices shall be connected only to scalar nets or bit-selects of vector
  nets."
- Arrays of instances, SV 28.3.6 L46936-46943: "For each port or terminal
  where the bit length of the instance-array port expression is the same as
  the bit length of the single-instance port, the instance-array port
  expression shall be connected to each single-instance port." "If bit lengths
  are different, each instance shall get a part-select of the port expression,
  of a bit length equal to the instance port bit length. The least significant
  bit of the port expression shall be connected to the instance corresponding
  to the right-hand index of the array range." V 7.1.6 L6005-6006: "If bit
  lengths are different, each instance shall get a part-select of the port
  expression as specified in the range, starting with the right-hand index."
- Nonresistive devices, SV 28.13 L48297-48298 (V 7.11 L7385-7386): "The tran,
  tranif0, and tranif1 switches shall not affect signal strength across the
  bidirectional terminals, except that a supply strength shall be reduced to a
  strong strength."
- Resistive devices, SV 28.14 L48303-48325, Table 28-8 (V 7.12 L7390-7410,
  Table 38): "The rnmos, rpmos, rcmos, rtran, rtranif1, and rtranif0 devices
  shall reduce the strength of signals that pass through them according to
  Table 28-8." Supply drive and strong drive become pull drive, pull drive
  becomes weak drive, large capacitor and weak drive become medium capacitor,
  medium and small capacitor become small capacitor, high impedance stays
  high impedance.
- Combination, SV 28.12.1 L47610-47612: "If two or more signals of unequal
  strength combine in a wired net configuration, the stronger signal shall
  dominate all the weaker drivers and determine the result. The combination of
  two or more signals of like value shall result in the same value with the
  greater of all the strengths." 28.12.2 L47654-47656: "When two signals of
  equal strength and opposite value combine, the result shall be a value x,
  along with the strength levels of both signals and all the smaller strength
  levels."
- Net strengths, SV 28.15.1 L48334-48335: "In the absence of an overriding
  source, such a signal shall have a value 0 and a pull strength." 28.15.3
  L48360: "The supply0 and supply1 net types shall have supply driving
  strengths."
- Force, SV 10.6.2 L13397-13399: "A force procedural statement on a net shall
  override all drivers of the net—gate outputs, module outputs, and continuous
  assignments—until a release procedural statement is executed on the net.
  When released, the net shall immediately be assigned the value determined
  by the drivers of the net."
- Net delay, SV 10.3.3 L12911-12912: "any value change that is to be applied
  to wireA by some other statement shall be delayed for ten time units before
  it takes effect."

`%v` prints one mnemonic for a single strength level and two digits (0-side
then 1-side for x; strongest then weakest for one value) for a range (V
17.1.1.5, SV 21.2.1.5).

## Fixtures

- `strengths`: each drive strength through one `tran` (unchanged, supply to
  strong) and one `rtran` (Table 28-8); a chain of two trans and four rtrans
  (St, St, St, Pu, We, Me, Sm); a supply1 net through a tran and five rtrans
  (Su, St, Pu, We, Me, Sm, Sm); a strong 1 against a supply0 net through a
  tran gives StX (the supply arrives as strong) and St0 once the strong
  driver turns off; tri0, tri1 and pullup sources (pull).
- `blocking` (A5-D2): Su1 and St0 at one node pass St1 through a tran and Pu1
  through an rtran; Su1 and St0 at the source of an rtran give Pu1 and the
  pull 0 on the far node loses.
- `unknown_controls` (A5-D1): one switch with an x, z, 1 and 0 control
  between differently driven nodes (StH, 36X, 651, 65X, PuH, ...), and
  networks where the enumeration matters: a node with St1 in both cases keeps
  St1; two unknown switches in series; an unknown St0 into a node held by a
  supply through a tran (StX: St0 meets St1 in one case only).
- `loops`: a tran loop, an rtran loop (each node reduced only along its
  shortest path: Pu, We, We), a switch to the same net, and opposing drivers
  on both sides of tran and rtran.
- `force` (A5-D3): forcing a node of a chain, forcing x in the middle of a
  network, forcing a vector bit, forcing z (which isolates its neighbours)
  and releasing each.
- `delays` (A5-D4): tranif1 #(3, 5), tranif1 #4, tranif0 #(2, 6) and rtranif0
  with min:typ:max delays (typical 2 and 5), sampled every time unit; each
  switch is unknown (StH, PuH) until its first delay elapses, x and z
  controls take the smaller delay, and a control change replaces a pending
  state change.
- `ports` (4.9.6, A5-D5): switches inside modules on inout ports; a port is
  no strength reduction, so a supply1 through a tran in a submodule arrives as
  St1. A net delay of 3 on the middle net of a chain delays only that net.
- `arrays`: `tran t[3:0]`, `tranif1 g[3:0]` and `tranif0 h[0:3]` connect bit i
  of each terminal to the instance that also gets bit i of the control; a
  1-bit terminal connects to every instance; bit-select terminals of a vector
  driven by pull buffers.
- `gates`: gates drive switch nodes and switch nodes drive a gate input and a
  tranif control; undriven networks stay HiZ.
- `events`: event controls on every node of a network count only value
  changes; two driver changes in one pass that end at the old value wake
  nothing.

## Negative cases

- `neg_mos`: MOS switches stay unsupported by design (located `unsupported:`).
- `neg_wired`: a `wand` node in a switch network is rejected with its
  location.
- `neg_wide_terminal`: a 2-bit terminal of a single switch (28.8).
- `neg_array_width`: a 3-bit terminal on an array of two switches (28.3.6).
- `neg_tran_delay`: a delay on `tran` (frontend, 28.8).
- `neg_variable_terminal`: a variable as a switch terminal (frontend).
