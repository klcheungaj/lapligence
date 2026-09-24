# Packed conditional policy

`packed_mux_policy.sv` is a public CLI regression for IEEE 1364-2001
§4.1.13 Table 28 and IEEE 1800-2009 §11.4.11 Table 11-20. It enumerates
one-bit branch states for known and ambiguous selectors, then probes constant,
runtime, generate-case, and 65-bit packed values. The `localparam` and
generate-case pair verifies that the ambiguous Z/Z table cell is X before
simulator lowering. For an ambiguous selector, equal 0, 1, or X branch bits
survive; differing bits and the Z/Z table cell produce X. Known selectors
still return the selected branch, including Z bits.

The Rust test runs this fixture and the held-out R10 Z/Z witness through both
optimizer modes under `--edition 2001` and `--edition 2009`.
