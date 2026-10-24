# Datatype edge fixtures

self-checking SystemVerilog datatype-edge cases.

Coverage: selected boundaries, including packed-state conversion and wide
  indices.
- Runs: applicable cases run in both optimizer modes.
- Oracle: self-checking output must match the expected fixture result.
- Limits: LRM-derived boundary behavior is intentionally narrower than a full
  datatype-conformance claim.

## Contracts

- Self-checking IEEE 1800-2009 boundaries: signed/unsigned indices, real
  conversions, two-state subprogram and aggregate storage, enum defaults and
  base-state/signedness, numeric size-cast provenance (source-enabled and
  source-less paths at the exercised widths), and near-limit storage combined
  with recursion.
- Ambiguous source-less cast provenance must diagnose explicitly; it must not
  silently change cast width or signedness.
- `packed_state_aggregates.sv` (§7.2.1): a packed struct with `bit` and `logic`
  members is four-state, while reads/writes through the named `bit` member
  convert X/Z to zero. Icarus Verilog 12.0 preserves those X/Z values; that
  divergence does not change the LRM oracle.
- `wide_lhs_indices.sv` (§7.4, §11.5.1): a 96-bit select index over data of at
  most 32 bits; a set bit above bit 63 makes the writes out of range and
  ineffective. Icarus 12.0 truncates the index and aliases a low element; the
  fixture keeps the LRM-derived no-op oracle.
- Oracles come from the local specification (sections indexed in
  `docs/specification/spec-reference-sv.md`), not implementation source.
