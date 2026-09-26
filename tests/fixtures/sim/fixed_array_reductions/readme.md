# Fixed unpacked-array reductions (R03)

All fixtures use the public CLI suite `tests/sim_fixed_array_reductions.rs` in
SystemVerilog-2009 mode, optimized and unoptimized. Sources are checked in;
expected output is in the Rust suite. Testbench initialization and display are
observation scaffolding, not claims that those entire modules are synthesizable.

| Fixture | Contract |
| --- | --- |
| `basic.sv` | Five runtime methods, optional parentheses, mutation after the first read. |
| `widths.sv` | Narrow overflow; explicit widened maps; named iterators; one-bit fill maps; all mapped operations. |
| `signed.sv` | Signed packed, enum and packed-record elements; conversion after the narrow fold. |
| `four_state.sv` | X/Z propagation, singleton preservation, two-state mapping and absorbing bitwise values. |
| `wide.sv` | Exact 65/129-bit values and widening before reduction. |
| `nested.sv` | Immediate rows, nested default/named iterators, outer capture and declared index queries. |
| `iterator_indices.sv` | Ascending/descending bounds, default and explicit dimension 1, dynamic dimension 1, and nested lexical iterator identities. |
| `iterator_index_zero.sv` | Reject a constant zero iterator dimension. |
| `iterator_index_out_of_range.sv` | Reject a packed item dimension that has no live iterator index. |
| `aggregates.sv` | Packed records and bounded unpacked-record/array-member maps. |
| `functions.sv` | One function receiver evaluation; automatic local/formal captures; unselected branch suppression. |
| `views.sv` | Selected rows and slices with one selector evaluation and declared slice indices. |
| `ports.sv` | Remapped input bounds, runtime updates and combinational sensitivity. |
| `resizable_control.sv` | Existing dynamic, queue and associative reduction paths stay separate. |
| `unmapped_row.sv` | Reject an unpacked immediate element without an integral map. |
| `nonintegral_map.sv` | Reject a real-valued map. |
| `iterator_without_with.sv` | Reject a named iterator without a with expression. |

Handwritten C probes in `tests/runtime_value_storage` do not replace these HDL runs.
