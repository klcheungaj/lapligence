# FND-003 pilot

`packed_pattern.sv` uses IEEE 1800-2009 sections 7.2.1 (packed structures) and
10.9.2 (structure assignment patterns). Assignment converts each member to its
declared type: the two-state `flags` member converts X/Z to zero (section 6.11.2).
Copying the packed structure preserves `payload`'s X bits; subsequently writing
the original member leaves the copy unchanged.

The independent oracle in `packed_pattern.out` is
`copied=ax flags=00 original=34`. The Rust task module runs the public CLI and
the validated owned pipeline after snapshot/Db destruction, each with both HDL
optimizer modes and native O0/O3. The checker gate is a separate component test.
This pilot preserves existing support and does not change a feature marker.
