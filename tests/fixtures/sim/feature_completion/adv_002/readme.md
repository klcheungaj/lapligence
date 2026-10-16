# ADV-002 distributed continuous, gate and net delays

Every expected output is hand-derived from the clauses quoted below; none is
produced by llg. The Verilog (`.v`) sources run in both editions
(`--edition v2001` and `sv2009`), every positive source runs in both optimizer
modes on the legacy, compact/portable and compact/GMP value backends
(`tests/sim_feature_completion/adv_002.rs`). The portable decisions are the
`S2-D*` cases in `tests/fixtures/sim/lrm_decisions/` (see
`docs/lrm_decisions.md`).

## Clauses

Line numbers are those of the `pdftotext -layout` extractions
`Verilog-1364-2001.txt` (V) and `SystemVerilog-1800-2009.txt` (SV).

- Continuous-assignment delays, V 6.1.3 L5563-5573 (SV 10.3.3 L12894-12903):
  "A delay given to a continuous assignment shall specify the time duration
  between a right-hand side operand value change and the assignment made to
  the left-hand side. If the left-hand side references a scalar net, then the
  delay shall be treated in the same way as for gate delays ... If the
  left-hand side references a vector net, then up to three delays can be
  applied. The following rules determine which delay controls the assignment:
  — If the right-hand side makes a transition from nonzero to zero, then the
  falling delay shall be used. — If the right-hand side makes a transition to
  z, then the turn-off delay shall be used. — For all other cases, the rising
  delay shall be used."
- Net delays and declaration assignments, V 6.1.3 L5581-5585 (SV L12911-12915):
  "This syntax, called a net delay, means that any value change that is to be
  applied to wireA by some other statement shall be delayed for ten time units
  before it takes effect. When there is a continuous assignment in a
  declaration, the delay is part of the continuous assignment and is not a net
  delay. Thus, it shall not be added to the delay of other drivers on the net.
  Furthermore, if the assignment is to a vector net, then the rising and
  falling delays shall not be applied to the individual bits if the assignment
  is included in the declaration."
- Inertial replacement, V 6.1.3 L5587-5600 (SV L12917-12926): "a) The value of
  the right-hand side expression is evaluated. b) If this RHS value differs
  from the value currently scheduled to propagate to the left-hand side, then
  the currently scheduled propagation event is descheduled. c) If the new RHS
  value equals the current left-hand side value, no event is scheduled. d) If
  the new RHS value differs from the current LHS value, a delay is calculated
  in the standard way using the current value of the left-hand side, the newly
  calculated value of the right-hand side, and the delays indicated on the
  statement; a new propagation event is then scheduled to occur delay time
  units in the future."
- Gate and net delays, V 7.14 L7456-7471 (SV 28.16 L48369-48384): "Net delays
  refer to the time it takes from any driver on the net changing value to the
  time when the net value is updated and propagated further. ... When one delay
  value is given, then this value shall be used for all propagation delays
  associated with the gate or the net. When two delays are given, the first
  delay shall specify the rise delay and the second delay shall specify the
  fall delay. The delay when the signal changes to high impedance or to unknown
  shall be the lesser of the two delay values. ... When a value changes to the
  unknown (x) value, the delay is the smallest of the three delays." Table 39
  (SV Table 28-9) gives the from/to choice per transition.
- min:typ:max, V 4.3 L4772-4776 (SV 11.12 L16642-16651): "Verilog HDL models
  typically specify three values for delay expressions. The three values allow
  a design to be tested with minimum, typical, or maximum delay values. Values
  expressed in min:typ:max format can be used in expressions." V 7.14.1
  L7548-7550: "The syntax for delays on gate primitives (including
  user-defined primitives ...), nets, and continuous assignments shall allow
  three values each for the rising, falling, and turn-off delays. The minimum,
  typical, and maximum values for each delay shall be specified as constant
  expressions separated by colons."
- Net initialization, V 3.6 L2631-2632 (SV 6.7 L5199-5200): "The default
  initialization value for a net shall be the value z. Nets with drivers shall
  assume the output value of their drivers."

## Fixtures

- `mintypmax_select`: continuous, gate (`and`, `bufif1`), net, parameter,
  summed (`(1:2:3) + (4:5:6)`), procedural variable and procedural constant
  min:typ:max delays all take the typical value (S2-D1).
- `vector_transitions`: a whole vector and a two-bit part-select take one
  delay per change by the 6.1.3 rules (nonzero to zero falls, all-Z turns off,
  partly X or Z values rise); a one-bit select follows Table 39, so 0 to X
  takes the smallest delay (S2-D2). Selected targets start X (S2-D4).
- `net_delay_bits`: a vector net delay applies per bit, so bits rising and
  falling at once reach the net at their own rise and fall times and staggered
  changes are not merged (S2-D3); a scalar three-delay net follows Table 39.
- `decl_assign_delay`: a declaration assignment's delay delays only that
  driver, never the net's other driver, and on a vector it is one delay (S2-D5).
- `pulse_widths`: continuous, gate, UDP and net delays filter a pulse narrower
  than the delay, pass pulses as wide as or wider than the delay, and replace a
  pending 1 by X. Stimulus uses nonblocking assignments, so an update maturing
  at the time of the next input change runs first (SV 4.4.2): an exactly
  delay-wide pulse propagates.
- `gate_forms`: one, two and three gate delays (`and` instance array, `not`,
  `bufif0`, `notif1`), turn-off and to-X choices; each gate-array instance
  output starts X.
- `net_array_delays`: a declaration delay on a one- and a two-dimensional net
  array delays every driven element after resolution, adds to a delayed
  driver's own delay, filters a 1 ns pulse and applies per bit to vector
  elements, including one driven through a bit-select.
- `net_array_ports` (SystemVerilog): the delayed element reached through an
  inout port and through an `alias` of one element is delayed once.
