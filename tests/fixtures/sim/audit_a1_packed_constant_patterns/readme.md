# A1 packed integral constant patterns

`sim_audit_a1_packed_constant_patterns` runs these checked-in SV2009 sources
through the public `llg` CLI in both optimizer modes. Runtime plusargs keep
the matched packed values from being compile-time-only witnesses.

The independent oracles follow IEEE 1800-2009 §§7.2.1, 7.3.1 and 12.6:
packed structs and untagged packed unions are integral vectors; a constant
pattern has the matched type and compares the complete value. Section 12.5.1
and §12.6.1 make `casez` ignore Z and `casex` ignore X/Z on either operand.
Section 7.2.1 converts reads of two-state members within a four-state packed
record, while four-state members retain X/Z.

`whole_values.sv` checks runtime true/false whole-struct and whole-union
comparisons. `contexts.sv` checks nested struct and tagged payload constants,
72-bit width, signed packed matching, mixed-state members
and packed-struct case modes. The negative
sources test the §12.6 integral-type rule for a real constant and the
matched-type rule for an unpacked structure subject.
