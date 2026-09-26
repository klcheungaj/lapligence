# SYN-026 fixed-array iterator indices

`indices.sv` exercises public CLI execution in SystemVerilog-2009 mode with
optimization enabled and disabled. IEEE 1364-2001 has no array manipulation
methods, so it has no applicable positive cell. Expected values are in
`tests/sim_syn026_iterator_indices.rs`.

| Cell | Public fixture and oracle |
| --- | --- |
| Default and explicit dimension 1, signed declared coordinates | `indices.sv`: bare `index` on ascending `[2:4]` sums to 9; `index(1)` on descending `[3:1]` sums to 6. The earlier `fixed_array_reductions/iterator_indices.sv` covers `index()` and runtime dimension 1, checking one dimension-function call per item. |
| Distinct nested iterators | `indices.sv`: two-dimensional `[1:0][-2:-1]` gives -4. Three-dimensional `[1:0][-1:0][3:4]` gives `4*(1+0) + 4*(-1+0) + 4*(3+4) = 28`; each query refers to its named lexical iterator. |
| Ordering `with` keys | `indices.sv`: `rsort` by ascending `[2:4]` indices and `sort` by descending `[3:1]` indices reverse their values. Sorting `[1:0]` row indices swaps complete rows. |
| Runtime dimension, receiver/map evaluation and formal bounds | `dynamic_formal.sv`: three mapped values sum to `(10+2)+(20+3)+(30+4)=69`; the receiver runs once, the dynamic dimension expression runs once per mapped item plus once as a formal actual, and the mapper runs once per item. A fixed-array formal with `[2:4]` bounds returns index sum 9. Three `int` query widths sum to 96 bits. |
| Undefined constant dimension | Existing `fixed_array_reductions/iterator_index_zero.sv` and `iterator_index_out_of_range.sv` reject `index(0)` and `index(2)` with `undefined dimension` diagnostics. Current public CLI diagnostics are from codegen. These are negative cells. |
| Unvisited unpacked dimension | `unvisited_dimension.sv` rejects `row.index(2)` on an outer iterator over a two-dimensional array: only the current row coordinate is live. This is a codegen-diagnosed negative. |
| Owned capture | `tests/slang_semantics/fixed_reductions.rs` checks the two distinct named iterators and their `[1:0][-2:-1]` / `[-2:-1]` receiver bounds, then lowers their index queries after the native snapshot is dropped. |
| Edition boundary | `edition_boundary.sv` uses Verilog-2001 declarations and only adds the SV-2009 array method/iterator query. It prints 9 in SV-2009 and receives a frontend syntax diagnostic in 2001. |
| Fixed-array locators | Unselected: the fixed locator family remains unsupported, as recorded in `docs/sim_features.md`. |

Testbench initialization and display are observation scaffolding. The fixed
iterator has one live dimension per method call; nested method calls introduce
separate iterator identities rather than exposing unvisited dimensions of an
outer iterator.
