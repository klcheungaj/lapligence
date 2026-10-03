# RTL-009 fixed value, reference and net-array ports

IEEE 1364-2001 §12.3 and IEEE 1800-2009 §§6.5, 6.22.2, 7.4.6, 7.6, 23.3.3.2–23.3.3.7
and 25.5 supply the oracles. Every positive source runs through the public CLI
in both optimizer modes, on the legacy and compact value backends, and after
snapshot/Db destruction.

- `module_interface_generate` connects interface modport arrays (read, written
  from a generate loop and forwarded as a whole fixed-array actual), generated
  instances and an instance array whose array ports distribute leftmost element
  to leftmost instance (§23.3.3.5).
- `computed_inputs` passes reversed-bound arrays, patterns with calls, slices,
  conditional arrays, a runtime-selected row and a cast row; contents and
  selector changes re-evaluate the link.
- `nested_output_members` publishes fixed outputs into nested record members,
  member arrays and constant rows of a two-dimensional array.
- `state_conversions`: a two-state cast before a four-state formal keeps the
  X/Z-to-0 conversion, for scalars, packed records, widened values and arrays.
- `ref_value_ports`: ref formals are the actual variables (writes are visible
  through hierarchical reads before any update event), while value inputs are
  converted copies updated by their implied continuous assignment.
- `output_runtime_select` adopts FND-002's L-F03-06-04 witness. With
  `runtime_output_targets`, a runtime-selected variable output is an implied
  continuous assignment: a selector change retargets the write, the previous
  element keeps its value and an unknown selector writes nothing (§7.4.6).
- `net_array_ports` covers net-array input/output ports, same-direction slices,
  and inout net arrays whose cells resolve with all connected drivers.
- `descriptor_ports` uses 65,537-cell arrays: whole copies, runtime-selected
  rows, converting casts, conditionals, reversed bounds and ref sharing all use
  descriptor copies; the test also bounds the generated model size.
- Negatives: an output expression that is not assignable and a chandle port
  (adopted witnesses), an inequivalent ref shape, a variable on an inout port,
  runtime-selected ref, net output and inout connections, and a variable output
  target that is also written procedurally or by a second output port.

All `.out` files are hand-derived from the clauses above.
