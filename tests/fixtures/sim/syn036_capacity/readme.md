# SYN-036 capacity boundaries

These fixtures separate the selected generated-model resource limits from the
fixed-array syntax described by IEEE 1364-2001 §§3.3.1 and 3.10 and IEEE
1800-2009 §7.4.2. The standards describe vector and unpacked-array forms but do
not set this implementation's storage ceiling.

`cell_limit_below.sv`, `cell_limit_at.sv`, and `cell_limit.sv` probe 65,535,
65,536, and 65,537 cells in both editions. `packed_width_below.sv`,
`packed_width_at.sv`, and `packed_width_above.sv` probe 1,048,574,
1,048,575, and 1,048,576 bits per packed array element. `cellwise_reduction.sv` uses 65,536
17-bit cells (1,114,112 flattened bits), above the 1,048,575-bit packed value
capacity, while reading owned cells one at a time. `aggregate_value_limit.sv`
uses that same bounded cell count in a formal; its flattened payload is rejected
with a resource diagnostic. `flat_value_below.sv` and `flat_value_at.sv` keep
fixed-value formals/returns at 1,048,574 and 1,048,575 bits. The smaller
`nested_fixed_value.sv` checks two-dimensional value copies separately from
cell-wise reductions. `recursion_boundary.sv` admits 255 and 256 active calls;
`recursion_guard.sv` exercises the 257th-call diagnostic. All large generated
models are ignored by the fast lane and run with `--run-ignored only`.

Dimension-product, projection-stride and flattened-width overflow checks have focused Rust
unit tests. No case enables unbounded or automatically resizable aggregates.
