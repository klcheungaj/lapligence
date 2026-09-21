# Packed conditional policy

`packed_mux_policy.sv` is a public CLI regression for IEEE 1364-2001
§4.1.13 Table 28 and IEEE 1800-2009 §11.4.11 Table 11-20. It exhaustively
enumerates one-bit branch states for known and ambiguous selectors, then probes
constant, runtime, and 65-bit packed values. The constant pair covers both the
owned optimizer path and the frontend materialized parameter path. For an
ambiguous selector, equal known `0`/`1` branch bits survive; branch X/Z bits,
including Z/Z, become X.

The Rust test runs the fixture through both optimizer modes under `--edition
2001` and `--edition 2009`.
