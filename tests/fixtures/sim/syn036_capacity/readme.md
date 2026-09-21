# SYN-036 capacity boundaries

These fixtures separate the selected generated-model resource limits from the
fixed-array syntax described by IEEE 1364-2001 §§3.3.1 and 3.10 and IEEE
1800-2009 §7.4.2. The standards describe vector and unpacked-array forms but do
not set this implementation's storage ceiling.

`cellwise_reduction.sv` uses 65,536 17-bit cells (1,114,112 flattened bits),
which is above the 1,048,575-bit packed value capacity. The direct rank-one
reduction remains legal because the generated model reads owned cells one at a
time. `cell_limit.sv` is one cell above the selected fixed-array storage limit.
`aggregate_value_limit.sv` keeps the same bounded cell count in a value formal,
where one flattened packed payload is required and is rejected with a capacity
diagnostic. None of these cases enables unbounded or automatically resizable
aggregate values.
