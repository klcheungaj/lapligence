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
  at `$+1` or the size still appends. `$` arithmetic, `insert` and `delete`
  with wide or X indices (no effect) and queue slices (bounds of any width
  clip; an X bound gives the empty queue) are included. Queues of `int`, 4-state, `string`, `real`, records, a record
  member and a class property are covered.
- `wide_index_strings.sv`: string indices of the same kinds, and the index
  arguments of `getc`, `putc` and `substr`.

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
  of any width (`longint`, a 65-bit signed negative, a 33-bit and a 65-bit
  positive value); an X bound gives the empty queue (L9039).
- 6.16.2 L5799-5803: "function void putc(int i, byte c);" and "If i < 0 or i >=
  str.len(), then str is unchanged." 6.16.3 L5809-5812: "If i < 0 or i >=
  str.len(), then str.getc(i) returns 0." 6.16.8 L5859-5863: "If i < 0, j < i,
  or j >= str.len(), substr() returns " " (the empty string)." The `int`
  formals do not reduce a wider actual to its low 32 bits: the index is out of
  range, so `getc` returns 0, `putc` changes nothing and `substr` is empty.
  L5805 and L5814 call `putc` and `getc` "semantically equivalent" to the
  select forms, which agrees.
- 7.10.2.2 L9088-9090 (`insert(input int index, input element_t item)`) and
  7.10.2.3 L9094-9099 (`delete([int index])`) do not say what an invalid index
  does; 7.10.1 does (no operation), so a wide or X index changes nothing.
