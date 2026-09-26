# SYN-021 finite tagged-union qualification

`struct_contexts.sv` exercises a 14-bit packed tagged union with void,
packed-structure and primitive members. It checks the initially unknown tag,
unequal payload widths, equal payload bits with different tags, declaration and
conditional-arm constructors, value copies, signed member reads through
concat/cast/formal conversion,
input/output/inout/const-ref copies, a persistent static function local, a ref
receiver and a selected NBA whose index changes after issue. The oracle follows
IEEE 1800-2009 §§7.3.2, 11.9 and 4.9.4. Both optimizer modes run through the
public CLI in edition 2009. The source is rejected in edition 2001.

`invalid_member_constructor.sv`, `invalid_void_constructor.sv` and
`nonpacked_member.sv` are separate 2009 frontend negatives under §§11.9 and
7.3.2. `nested_inactive_read.sv` and `nested_inactive_write.sv` check both
levels of the tag guard with exact source-addressed runtime errors, preserved
active payloads and one receiver evaluation. `nba_wrong_at_issue.sv` checks
the issue-time target rule even when a later blocking assignment changes the
tag before the NBA update. These runtime cases run in both optimizer modes.

The companion `sim_tagged_union_access` and `sim_data_types_next` suites cover
other wrong-tag diagnostics, nested tagged payloads, value/port contexts and
unsupported unpacked tagged storage. Q03 remains open only for a valid issue
target whose tag changes before NBA publication; no commit-time recheck oracle
is asserted for that case.
