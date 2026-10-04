# RTL-099 fixed RTL gate regressions

IEEE 1800-2009 §§6.5, 9.2.2.4 and 23.3.3.2 supply the oracles: an output port
connected to a variable is an implied continuous assignment, so it is the one
continuous writer of exactly the cells its actual names, and any other
ordinary writer of those cells is an error. Expected values are derived by
hand in the comments below; none were captured from `llg`.

- `descriptor_slice_writers`: output ports drive constant slices `a[0:3]` and
  `a[65533:65536]` of a 65,537-cell array and the two-row slice `b[2:3]` of a
  4,096 x 16 array (both descriptor-backed). Procedural writes to the
  neighbouring cells `a[4]`, `a[65532]`, `b[1][15]` and `b[4][0]` stay legal.
  Port values follow `base` (`10..13`, then `20..23`), the fixed `8'h80` base
  (`80..83`) and `8'h40 + 16*r + c` (`b[2][0] = 40`, `b[3][15] = 5f`); the
  never-written `a[5]` keeps its four-state default `xx`.
- `neg_descriptor_slice_procedural`: an initial write to `a[1]` overlaps the
  port-driven slice `a[0:3]`.
- `neg_descriptor_slice_ports`: two output ports drive the overlapping slices
  `a[0:3]` and `a[3:6]`.
- `neg_descriptor_rows_ff`: an always_ff write to `b[3][7]` overlaps the
  port-driven rows `b[2:3]`.
- `record_array_values` (A02 qualification): an array of 1,048,576 unpacked
  records with a four-state 16-bit and a two-state 4-bit member (20,971,520
  bits, beyond the packed limit) supports member writes, a whole copy, `==`
  and `!=` (the known mismatch in element 9 decides `0`/`1` despite X cells),
  an ambiguous conditional, a function input/return and an NBA. Element 7 is
  equal in both arms and survives the X selector; element 9 differs, so the
  whole record takes its uninitialized default (`xxxx`, two-state `0`) as the
  §11.4.11 merge of an unpacked element. The NBA publishes `bump(m)`: element
  7 incremented to `0011`, element 9 and the last element copied from `m`.
  The test also bounds the generated `model.c`.
