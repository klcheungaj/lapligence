# Random-stream simulator fixtures

`random_streams.sv` covers the H07 SystemVerilog 2009 random facilities:
`$urandom` (including its input seed), `$urandom_range`, process stream
seeding/state replay, and fork stream isolation. The assertions intentionally
check relationships and range invariants rather than pinning the
implementation's unspecified numeric sequence.

The fixture is compiled and executed with both optimizer configurations by
`tests/sim_random_streams.rs`. Its source-header anchors are §18.13 and
§18.14 of the local IEEE 1800-2009 specification.
