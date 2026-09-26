# SYN-024 tagged pattern acceptance

IEEE 1800-2009 §§7.3.2, 11.9 and 12.6 define the finite packed tagged
union and pattern forms exercised here. `tagged_runtime.sv` checks whole-value
wildcard and binding, tag-first void and active payload matching, recursive
instruction-like structure and tagged payloads, binding scope through `&&&`,
single source capture, X/Z data, and both conditional arms. Each assertion uses
a specified tag/payload value, and the Rust suite checks exact stdout in both
optimizer modes.

`case_modes.sv` applies the §12.6.1 enclosing `case`/`casez`/`casex` mode to
tag and payload bits. X is not a `casez` wildcard, Z is; both are `casex`
wildcards. Ordinary `if ... matches` and checked member access retain exact
active-tag behavior. The neighboring `sim_tagged_union_access` suite checks
wrong-tag member access diagnostics. `bad_tag_name.sv` and
`bad_source_type.sv` are single-fault frontend controls. `edition_boundary.sv`
checks rejection in IEEE 1364-2001, which has no tagged pattern syntax.

Q03, a valid tagged NBA target retagged before publication, remains open in
the SYN-021 storage contract; these predicate fixtures make no timing claim.
