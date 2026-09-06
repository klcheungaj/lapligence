# Datatype edge fixture contracts

These self-checking fixtures target IEEE 1800-2009 datatype boundaries. The
Rust harness runs applicable cases with optimization disabled and enabled.

The edge campaign covers signed and unsigned indices, real conversions,
two-state subprogram and aggregate storage, enum defaults, numeric size-cast
provenance, and near-limit storage combined with recursion. Enum base-state and
signedness behavior plus numeric source-enabled and source-less cast paths are
covered at the exercised widths. Ambiguous source-less cast provenance must
diagnose explicitly; it must not silently change cast width or signedness.

`packed_state_aggregates.sv` follows §7.2.1: a packed structure containing
`bit` and `logic` members is four-state, while reads/writes through the named
`bit` member convert X/Z to zero. Icarus Verilog 12.0 preserves those X/Z
values; that confirmed reference-tool divergence does not change the LRM oracle.

`wide_lhs_indices.sv` forms a 96-bit select index while stored data signals
remain at most 32 bits. Sections §7.4 and §11.5.1 require the self-determined
index width, so a set bit above bit 63 makes the writes out of range and
ineffective. Icarus 12.0 truncates the index and aliases a low element; the
fixture retains the LRM-derived no-op oracle.

The cited sections are indexed in `docs/specification/spec-reference-sv.md`;
the HDL oracle is derived from the local specification, not implementation
source.
