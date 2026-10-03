# RTL-010 fixed continuous assignment topology

IEEE 1364-2005 §6.1 and IEEE 1800-2009 §§6.5, 7.4, 10.3, 10.9, 10.11 and
Table 10-1 supply the oracles. Every positive source runs through the public
CLI in both optimizer modes, on the legacy and compact value backends, and
after snapshot/Db destruction.

- `alias_patterns`: a positional pattern leaf drives selected bits of one
  `alias` view while independent drivers on the other view compete with it;
  an ordinary net has two pattern leaves and an overlapping selected driver.
  The runtime RHS changes between steps.
- `net_array_patterns`: pattern leaves select bits inside net-array cells,
  name a whole net array, and name a true alias of one cell, next to an
  independent whole-cell driver.
- `nested_targets`: nested patterns publish into packed nets, an
  unpacked-structure net and its members, and disjoint variable members and
  cells. `net_pattern_driver` adopts FND-002's L-F08-02-03 witness.
- `variable_writers`: disjoint rows, cells, unpacked members, packed member
  ranges and a hierarchical variable each keep one continuous writer beside
  procedural writers of the other parts.
- `time_zero_notifications`: drivers settle in time slot zero (`$strobe`), and
  readers of each variable, net, alias and structure-member leaf wake only when
  that leaf changes. Time-zero wakeups are excluded because their order against
  the counting processes is not fixed.
- `zero_time_feedback`: drivers that read their own targets (directly, through
  another alias view, or through array cells) re-evaluate until they settle;
  `neg_feedback_nonconvergent` never settles and stops at the per-process
  zero-time step limit.
- `descriptor_scatter`: 65,537-cell rows scatter from a two-dimensional array,
  from typed patterns with array and `default` items, and from a pattern that
  reads one of its own leaves; the test also bounds the generated model size.
- `composition`: parameterized and generated instances combine aliases,
  net-array cells and pattern drivers that read other pattern-driven nets, at
  4 and 65 bits.
- Negatives: overlapping continuous, procedural, initializer and hierarchical
  writers of members, rows, cells and pattern leaves (including FND-002's
  `neg_continuous_extra_writer`), and a runtime-selected pattern leaf.
- `boundary_delayed_pattern` adopts FND-002's legal L-F08-02-02 witness. Delayed
  pattern drivers belong to ADV-002 and remain rejected until then.

All `.out` files are hand-derived from the clauses above.
