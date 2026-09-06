# Datatype edge fixtures

These self-checking SystemVerilog fixtures exercise selected datatype boundaries
against the IEEE 1800-2009 language rules in `docs/specification/`. The Rust
harness runs applicable fixtures with optimization disabled and enabled.

`packed_state_aggregates.sv` follows IEEE 1800-2009 §7.2.1 for a packed
structure that contains both `bit` and `logic` members. The aggregate is
four-state, but reading its `bit` member implicitly converts four-state storage
to two-state, and writing that member converts the two-state member value back
to the aggregate's four-state representation. Consequently, X/Z values read
through or assigned into the named `bit` member are observed as zero. Icarus
Verilog 12.0 preserves those X/Z values instead; that reference-simulator
deviation does not change the LRM-derived oracle.

`wide_lhs_indices.sv` keeps every stored data signal at 32 bits or less while
forming a 96-bit select index by concatenation. IEEE 1800-2009 §§7.4 and
11.5.1 require the index expression to retain that self-determined width, so a
set bit above bit 63 makes the packed and unpacked writes out of range and
therefore ineffective. Icarus Verilog 12.0 instead truncates the index and
aliases the low in-range element; the fixture retains the LRM-derived no-op
oracle.
