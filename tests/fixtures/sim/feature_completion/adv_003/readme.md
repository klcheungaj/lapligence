# ADV-003 sequential UDP tables and initialization

Every expected output is hand-derived from the clauses quoted below; none is
produced by llg. All sources are Verilog (`.v`) and run in both editions
(`--edition v2001` and `sv2009`); every positive source runs in both optimizer
modes on the legacy, compact/portable and compact/GMP value backends
(`tests/sim_feature_completion/adv_003.rs`). The portable decisions are the
`S3-D*` cases in `tests/fixtures/sim/lrm_decisions/` (see
`docs/lrm_decisions.md`).

## Clauses

Line numbers are those of the `pdftotext -layout` extractions
`Verilog-1364-2001.txt` (V) and `SystemVerilog-1800-2009.txt` (SV).

- State field and next state, V 8.1.4 L7876-7879 (SV 29.3.4 L48791-48794):
  "Sequential UDPs have an additional field inserted between the input fields
  and the output field. This additional field represents the current state of
  the UDP and is considered equivalent to the current output value. It is
  delimited by colons. Each row defines the output based on the current state,
  particular combinations of input values, and at most one input transition".
- Unmatched combinations, V 8.1.4 L7885-7886 (SV L48801-48802): "It is not
  necessary to explicitly specify every possible input combination. All
  combinations of input values that are not explicitly specified result in a
  default output state of x."
- Contradictory entries, V 8.1.4 L7888 (SV L48804): "It is illegal to have the
  same combination of inputs, including edges, specified for different
  outputs."
- Z inputs, V 8.1.5 L7892-7893 (SV 29.3.5 L48808-48809): "The z values passed
  to UDP inputs shall be treated the same as x values."
- Symbols, V 8.1.6 Table 40 L7912-7926 (SV Table 29-1 L48828-48862): "?
  Iteration of 0, 1, and x"; "b Iteration of 0 and 1"; "- No change Permitted
  only in the output field of a sequential UDP"; "(vw) Value change from v to
  w ... v and w can be any one of 0, 1, x, ?, or b, and are only permitted in
  the input field"; "* Same as (??) Any value change on input"; "r Same as
  (01)"; "f Same as (10)"; "p Iteration of (01), (0x) and (x1)"; "n Iteration
  of (10), (1x) and (x0)".
- Level-sensitive state, V 8.3 L8027-8028 (SV 29.5 L48949-48950): "The output
  value of the UDP is always the same as the internal state."
- Edge rows, V 8.4 L8037, L8042-8043 (SV 29.6 L48960, L48965-48966): "Each
  table entry can have a transition specification on at most one input." "All
  transitions that do not affect the output shall be explicitly specified.
  Otherwise, such transitions cause the value of the output to change to x.
  All unspecified transitions default to the output value x." V L8087-8088
  (SV L49007-49008): "The transition of clock from 0 to x with data equal to 0
  and current state equal to 1 will result in the output q going to x."
- Initial value, V 8.1.3 L7844 (SV 29.3.3 L48761): "The sequential UDP initial
  statement specifies the value of the output port when simulation begins."
  V 8.5 L8146-8148 (SV 29.7 L49064-49066): "The output q has an initial value
  of 1 at the start of the simulation; a delay specification on an
  instantiated UDP does not delay the simulation time of the assignment of this
  initial value to the output. When simulation starts, this value is the
  current state in the state table." V L8227-8229 (SV L49099-49101): "At
  simulation time 0, qi changes value to 1. That initial value of qi does not
  propagate to net q until simulation time 3, and it does not propagate to net
  qb until simulation time 5."
- Instances, V 8.6 L8262-8263 (SV 29.8 L49159-49161): "Only two delays can be
  specified because z is not supported for UDPs. An optional range may be
  specified for an array of UDP instances. The port connection rules remain
  the same as outlined in 7.1."
- Delays, V 7.14 L7459-7463 (SV 28.16 L48372-48376): "When two delays are
  given, the first delay shall specify the rise delay and the second delay
  shall specify the fall delay. The delay when the signal changes to high
  impedance or to unknown shall be the lesser of the two delay values."
- Mixed tables, V 8.7 L8289-8291 (SV 29.9 L49196-49198): "When the input
  changes, the edge-sensitive cases are processed first, followed by
  level-sensitive cases. Thus, when level-sensitive and edge-sensitive cases
  specify different output values, the result is specified by the
  level-sensitive case."
- Ports, V 8.1.1 L7823-7826 (SV 29.3.1 L48736-48739): "UDPs have multiple
  input ports and exactly one output port ... All ports of a UDP shall be
  scalar; vector ports are not permitted. The output port shall be the first
  port in the port list." V 8.1.2 L7834-7836 (SV L48747-48749): "Sequential
  UDPs shall contain a reg declaration for the output port ... Combinational
  UDPs cannot contain a reg declaration." V L7839: "they shall allow at least 9
  inputs for sequential UDPs".
- Scheduling, SV 4.4.2.3 L3152-3156: "The Inactive region holds the events to
  be evaluated after all the Active events are processed. ... an explicit #0
  delay control requires the process to be suspended and an event to be
  scheduled into the Inactive region of the current time slot".

## Fixtures

Each step of a stimulus changes one input unless noted, and the oracle applies
the clauses above to the inputs, the state before the change and the changed
input's old and new value (decision S3-D1).

| Fixture | Covers |
| --- | --- |
| `latch.v` | level rows with `0 1 x ? b` in input and state fields, `-`, an input Z read as X, unmatched X |
| `edge_symbols.v` | `r f p n *`, explicit `(vw)` with `0 1 x ? b`, `(bx)`, the X default of unspecified transitions; the input visits all twelve changes between 0, 1, x and z (z <-> x is no change, S3-D3) |
| `flip_flop.v` | the 8.4 `d_edge_ff` with `(01) (0?) (?0) (??)`, the clause's 0 -> x case and unspecified x -> 1 and 1 -> z |
| `mixed_tables.v` | the 8.7 `jk_edge_ff` (preset/clear level rows, clock and j/k edge rows) and a table where a level row overrides an edge row for the same rising clock |
| `initial_delay.v` | 8.5 examples 1 and 2 (Figure 30 timing), `output reg q = 0`, an undelayed initial value on a `#(4, 6)` instance, then rise, fall and to-x delays |
| `instance_array.v` | instance-array elements and separate instances keep independent states; scalar actuals fan out to every element |
| `same_slot.v` | changes separated by `#0` are separate transitions; two inputs changed in one step follow S3-D2 |
| `strengths.v` | drive strengths, resolution with other drivers, an X state at its strength, a delayed instance with strengths |
| `time_zero.v` | S3-D4: constant and supply inputs are transitions from x at time zero, undriven inputs are not |
| `wide_table.v` | ten inputs with four edge inputs: above the dense next-state limit, so the row-scan step runs; level dominance and level-only inputs there |

Negative fixtures (`neg_*.v`) each contain one fault and must stop with the
frontend's located diagnostic before any C is generated: transitions, a state
field, `-` or `initial` in a combinational UDP; a sequential row without a
state field or with two transitions; contradictory edge entries and a `-`
entry contradicting an explicit one; table and instance arity; output not
first; `reg` on an input; an initial value that is not a one-bit literal or
does not name the output; a vector actual on a single instance (llg's own
lowering error, which names the instance and its location).
