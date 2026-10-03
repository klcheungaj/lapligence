# Container element select fixtures

These fixtures cover bit, part and indexed part-select reads and writes of
packed elements of dynamic arrays, queues and associative arrays (IEEE
1800-2009 §7.4.6, §7.5, §7.8, §7.10, §11.5.1). Elements are wider than 64
bits, and X/Z values are written and read through the selects.

- `dynamic_selects.sv`: constant and non-constant element and bit indices,
  ascending and multidimensional packed elements, two-state and signed
  elements, and writes through invalid element or bit indices.
- `queue_selects.sv`: `$` and non-constant indices, a select write at `$+1`
  that appends, and an ignored write past `$+1`.
- `assoc_selects.sv`: integral and string keys, missing keys created from the
  type default or the array's specified default, and two-state elements.
- `nested_selects.sv`: a select of an element of a nested dynamic array.
- `compound_select.sv`: the rejected compound-assignment form.

A select write is one read/modify/write of the element: container indices
and the right-hand side are evaluated once, then the whole-element store
applies its usual invalid-index, append and key-creation rules.
`tests/sim_container_selects.rs` runs each positive fixture on both value
backends in both optimizer modes and compares exact output whose values were
derived by hand from the written bit patterns. `%h` fields that would mix X or
Z with known bits inside one hex digit are printed with `%b` instead.
