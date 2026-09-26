# Random-stream fixture contracts

`sim_random_streams.rs` runs `random_streams.sv` in both optimizer modes. Check
`$urandom` seed input, `$urandom_range`, process seed/state replay and fork isolation
through relationships/range invariants, not an unspecified numeric sequence.
Source anchors: local IEEE 1800-2009 §§18.13–18.14 (H07).
