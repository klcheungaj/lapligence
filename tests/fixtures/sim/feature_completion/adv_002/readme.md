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
- Concatenation and pattern targets, V 6.1 Table 30 L5395-5399: continuous
  assignment left-hand sides are a "Net (vector or scalar)", constant bit,
  part and indexed part selects of a vector net, and a "Concatenation of any
  of the above four LHS". SV 11.4.12 L15235-15236: "The concatenation is
  treated as a packed vector of bits. It can be used on the left-hand side of
  an assignment or in an expression." SV 10.9 L13617-13619: "When an
  assignment pattern expression is used in a left-hand expression, the
  positional notation shall be required; and each member expression shall
  have a bit-stream data type that is assignment compatible with and has the
  same number of bits as the corresponding element in the data type of the
  assignment pattern expression."
- Runtime-valued delays, SV 28.16.1 L48462-48466: "The minimum, typical, and
  maximum values for each delay shall be specified as expressions separated
  by colons. ... These can be any three expressions." V 7.14.1 L7548-7552
  requires "constant expressions separated by colons" instead.
- Aliases, SV 10.11 L13981: "An alias statement declares multiple names for
  the same physical net, or bits within a net."
- Net initialization, V 3.6 L2631-2632 (SV 6.7 L5199-5200): "The default
  initialization value for a net shall be the value z. Nets with drivers shall
  assume the output value of their drivers."

## Fixtures

- `mintypmax_select`: continuous, gate (`and`, `bufif1`), net, parameter,
  summed (`(1:2:3) + (4:5:6)`), procedural variable and procedural constant
  min:typ:max delays all take the typical value (S2-D1).
- `delay_select`: one min:typ:max delay of every form (continuous assignment,
  gate, UDP, net, parameter, specify path with a specparam, procedural
  constant and run-time delays) and a parameter expression
  `(1:2:3) + (10:20:30)`, run with `--delay-select min`, `typ` and `max`. With
  delays 1-6, 11-16 and 21-26 (in declaration order) each output falls from X
  to 0 at its delay after `a = 0` at time 0 and rises at 40 plus its delay;
  the procedural delays then end at 81/82/83 (`#(d1:d2:d3)` with 1, 2, 3) and
  82/84/86 (`#(1:2:3)`); `Q` is 11, 22 or 33. Expected outputs
  `delay_select_<min|typ|max>.out` are derived from those delays alone.
- `vector_transitions`: a whole vector and a two-bit part-select take one
  delay per change by the 6.1.3 rules (nonzero to zero falls, all-Z turns off,
  partly X or Z values rise); a one-bit select follows Table 39, so 0 to X
  takes the smallest delay (S2-D2). Selected targets start X (S2-D4).
- `net_delay_bits`: a vector net delay schedules the whole vector, its delay
  selected by the new least significant bit (0 falls, 1 rises), and a new
  value replaces the pending one (S2-D3); a scalar three-delay net follows
  Table 39. Driven delayed nets start x (S2-D10).
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
  driver's own delay, filters a 1 ns pulse and schedules each vector element
  as one vector, or a bit-select driver's part on its own (S2-D3).
- `net_array_ports` (SystemVerilog): the delayed element reached through an
  inout port and through an `alias` of one element is delayed once.
- `concat_targets`: a delayed concatenation is one vector driver (S2-D6): a
  nonzero-to-nonzero change rises in every part, a change to all-Z turns
  off, a newer value replaces the pending one as a whole, and a part with its
  own net delay adds it after the driver delay.
- `pattern_alias_targets` (SystemVerilog): each assignment-pattern member,
  including a net-array element, is its own delayed driver (S2-D7); a member
  and a whole target split over alias groups change together with one delay
  (S2-D6); a pulse narrower than the delay is filtered per member.
- `neg_delayed_descriptor_pattern` (SystemVerilog): a delayed pattern member
  that is a 65,537-cell variable row is rejected; such a row has no packed
  driver value.
- `neg_net_delay_negative`: a negative real net delay is rejected as a net
  propagation delay; a declaration assignment's negative delay is rejected
  as that driver's delay (`tests/sim_net_decl.rs`).
- `runtime_driver_delays` (SystemVerilog): integer, two-value, real,
  min:typ:max and X-valued continuous-assignment delays and runtime gate
  delays use the values current when the driver schedules; changing only a
  delay operand schedules nothing; X is zero (S2-D9).
- `runtime_net_delays` (SystemVerilog): a runtime net delay uses its value at
  each net change; on a vector net it selects the delay like a delayed
  continuous assignment, from the whole old and new value (S2-D9).
- `neg_runtime_delays_v2001`, `neg_runtime_gate_delay_v2001`,
  `neg_runtime_net_delay_v2001`: with `--edition v2001` runtime-valued
  continuous-assignment, gate and net delays are rejected (V 7.14.1).
- `neg_runtime_net_delay_conflict` (SystemVerilog): a runtime net delay
  aliased to a net with another declared delay is rejected (S2-D8).
