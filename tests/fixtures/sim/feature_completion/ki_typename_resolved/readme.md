# `$typename` of user-defined types

`tests/sim_feature_completion/ki_typename_resolved.rs` runs this design in both
optimizer modes and every value backend. Decisions
[TN-D1 and TN-D2](../../../../../docs/lrm_decisions.md#type-names-small-decisions).

Clause text is quoted from `docs/specification/SystemVerilog-1800-2009.txt`
with line numbers.

- L34538 (20.6.1): "The $typename system function returns a string that
  represents the resolved type of its argument."
- L34548: "a) A typedef that creates an equivalent type is resolved back to
  built-in or user-defined types."
- L34550: "c) System-generated names are created for anonymous structs,
  unions, and enums."
- L34552: "e) Actual encoded values are appended with enumeration named
  constants."
- L34562: "f) User-defined type names are prefixed with their defining package
  or scope name space."

The generated names of anonymous types (`e$1`, `s$2`) number them in the order
the frontend met them.
