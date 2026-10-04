# RTL-020 combinational UDP contexts

IEEE 1364-2001 §§7.1.5, 7.8-7.10, 8.1-8.2, 8.6 and IEEE 1800-2009 §§23.3.3.5,
28.3.6, 28.11-28.16, 29.3-29.4, 29.8 supply the oracles. Every expected value
was derived by hand from those clauses, except the exhaustive sweep, whose
expected text comes from the all-matching-rows oracle in
`tests/sim_feature_completion/rtl_020.rs` (a Z input reads as X, `b` covers
0/1, `?` covers 0/1/X, no match gives X). Positive sources run through the
public CLI in both optimizer modes and on the legacy and compact value
backends (set `LLG_TEST_GMP_ROOT` for the GMP lane); `.v` sources also run as
Verilog-2001.

- `exhaustive_tables.v` drives a mux, a parity table and a table using every
  combinational symbol (`0 1 x X b B ?` in, `0 1 x X` out, plus compatible
  overlapping rows) with all 64 combinations of 0/1/x/z, forward and then
  backward, so each combination is also reached from other states.
- `selected_terminals.sv` reads scalar inputs from bit, part and indexed
  selects, constant indices and parameters, unpacked and packed array
  elements, structure members, a 200,000-element array above the packed
  capacity (descriptor storage), hierarchical names, a function call, a
  conditional and a Z literal. Outputs drive a vector bit, unpacked net
  elements, a net-array cell bit and a net in another instance.
- `instance_arrays.v` connects UDP arrays to whole vectors, part-selects of
  wider nets (input and output), concatenations, literals, operator
  expressions numbered by their own type (`~a[3:2]`), a run-time indexed
  slice of a vector whose low bound is 1 and a replicated scalar.
- `instance_arrays_sv.sv` covers two-dimensional instance arrays, unpacked
  net-array connections matched dimension by dimension and a module instance
  array whose ports take part-selects of wider vectors.
- `drivers_delays.v` resolves competing UDP outputs on wire, wand, wor and
  tri1 nets, prints drive strengths with `%v` (`63X` is strong0/weak1 X), and
  checks `#(3,5)` rise/fall delays, the smaller delay for a transition to X,
  inertial cancellation and a parameter delay.
- `composition.sv` builds an 8-bit ripple adder from UDP instance arrays
  whose carry input feeds back a part-select of their own output, in two
  module instances with swapped operands; X and Z inputs propagate only where
  the majority table cannot resolve them.

Nearest-illegal negatives: `neg_vector_definition` (adopted FND-002 witness
`neg_udp_vector`, L-F08-09-02) and `neg_row_width` (adopted `neg_udp_bad_row`,
L-F08-09-01) reject vector definition ports and wrong row widths;
`neg_conflicting_rows`, `neg_terminal_count` and `neg_empty_terminal` reject
in both editions; `neg_vector_input`, `neg_vector_output`,
`neg_aggregate_input`, `neg_unpacked_input` and `neg_array_width` keep
vector, aggregate and mis-sized array terminals rejected, so accepting a
selected scalar never admits a vector terminal. Sequential tables remain
ADV-003's (`tests/sim_udp.rs`).
