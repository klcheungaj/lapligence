# RTL-013 fixed sensitivity and always writer rules

IEEE 1364-2001 §9.7.5 and IEEE 1800-2009 §§4.3, 6.5, 9.2.2.2–9.2.2.4, 10.6.2
and 23.3.3.3 supply the oracles. Every positive source runs through the public
CLI in both optimizer modes, on the legacy and compact (portable and GMP)
value backends, and most also after snapshot/Db destruction. Wake counters are
declaration-initialized, so the counted procedure stays their only writer, and
are compared only after the first sample where time-zero ordering is not fixed.

- `member_writers`: always_comb, always_latch, always_ff and continuous writers
  of disjoint nested record members, packed members, packed ranges and array
  cells coexist.
- `transitive_reads`: always_comb follows reads inside nested function calls;
  `@*` uses only call-site reads; constant selectors wake on one element and
  runtime selectors on the whole array.
- `ref_ports`: always_comb readers and always_ff writers through ref ports,
  nested two module levels deep, to whole variables, arrays, record members,
  packed members of nested records and array cells.
- `closed_latch`: a closed always_latch holds a scalar, an array cell and a
  record member and notifies nobody until it opens.
- `time_zero`: always_comb and always_latch run once at time zero after every
  initial and always procedure has started, including a procedure that reads
  nothing, so waiting readers see that first result exactly once.
- `ff_wake`: data changes alone never wake always_ff; an asynchronous reset
  or-list, `iff`, blocking local data, timing-free calls, delayed nonblocking
  assignments, event triggers and force/release are legal bodies.
- `override_writers`: force/release from another process on storage that
  always_ff or always_comb owns.
- `changed_only`: re-evaluation with identical results (a 65,537-cell
  descriptor copy, a dense copy, a record member, a packed range) notifies no
  reader.
- `descriptor_processes`: 65,537-cell descriptor arrays in always_comb, `@*`,
  called functions and always_ff, an always_comb that writes one element and
  reads the array, and disjoint writers of four 65,537-cell rows (continuous,
  always_comb, always_ff and an output port). The test bounds the generated
  model size.
- `string_sensitivity` and the adopted FND-002 witness `native_comb`: string
  variables and string record members wake always_comb and `@*` readers.
- `optimizer_wakes`: a branch pruned by elaboration constants keeps its wake
  sources in both optimizer modes.
- `composition`: generated, parameterized stages at 4 and 65 bits own disjoint
  cells and record members through ref ports while always_comb reductions and
  a called function read the composite.
- Negatives: blocking timing and forks in always_comb, a second event control
  (FND-002's `neg_ff_two_events`, an intra-assignment event), blocking timing
  in always_ff and in tasks called from always_ff/always_latch, and
  overlapping writers through members, cells, wide rows (procedural,
  continuous and output-port), ref ports, hierarchical references and called
  functions.

All `.out` files are hand-derived from the clauses above; `override_writers`
was also cross-checked with Icarus Verilog.
