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
- `wide_index_queue.sv`: queue element indices of 33, 64 and 65 bits, with
  low bits that name an existing element, an unsigned value above 2**31-1, an
  X index and a negative 64-bit value: reads return the default, writes and
  compound assignments are ignored, `ref` actuals are not bound, and a write
  at `$+1` or the size still appends. `$` arithmetic and queue slices (clipped
  bounds that fit, empty for a bound beyond 32 bits or an X bound) are
  included. Queues of `int`, 4-state, `string`, `real`, records, a record
  member and a class property are covered.
- `wide_index_strings.sv`: string indices of the same kinds, and the `int`
  formals of `getc` and `putc`, which convert a wide actual like any call
  argument.

Decision S07-D7 (`docs/lrm_decisions.md`) states the rule the two wide-index
fixtures pin. Clause lines (`SystemVerilog-1800-2009.txt`):

- 7.10.1 L9042-9047: "An invalid index value (i.e., a 4-state expression with
  X's or Z's, or a value that lies outside 0...$) shall cause a read operation
  (e = Q[n]) to return the default initial value for the type of queue item
  (as described in Table 7-1)." and "An invalid index (i.e., a 4-state
  expression with X's or Z's, or a value that lies outside 0...$+1) shall
  cause a write operation to be ignored and a run-time warning to be issued;
  however, writing to Q[$+1] is legal."
- 7.10.1 L9040-9041: "Q[ a : b ] where a < 0 is the same as Q[ 0 : b ]." and
  "Q[ a : b ] where b > $ is the same as Q[ a : $ ]." The fixture clips bounds
  that fit in 32 bits; a bound beyond 32 bits is not clipped and yields the
  empty queue, as an X bound does (L9039).
- 6.16.3 L5812: "If i < 0 or i >= str.len(), then str.getc(i) returns 0."
  L5809 declares `function byte getc(int i);` and L5814 says "The getc method
  assignment x = str.getc(j) is semantically equivalent to x = str[j].": the
  `int` formal converts a wide actual, so `getc(w33)` still reads element 3
  while `str[w33]` reads 0. The clauses do not settle which one wins; the
  fixture pins the current conversion for `getc` and `putc` (6.16.2 L5805).

A select write is one read/modify/write of the element: container indices
and the right-hand side are evaluated once, then the whole-element store
applies its usual invalid-index, append and key-creation rules.
`tests/sim_container_selects.rs` runs each positive fixture on both value
backends in both optimizer modes and compares exact output whose values were
derived by hand from the written bit patterns. `%h` fields that would mix X or
Z with known bits inside one hex digit are printed with `%b` instead.
