# RTL-016 fixed tagged unions, pattern matching and Q03

IEEE 1800-2009 §§7.3.2, 11.9 and 12.6 (with §§4.9.4 and 10.4.2 for
nonblocking assignments) supply the oracles. Positive sources run through the
public CLI in both optimizer modes and on the legacy and compact value
backends; representative sources also run after snapshot/Db destruction.

- `unpacked_payloads` covers every fixed payload category of an unpacked
  tagged union: void, two-state and four-state integral members, signed
  members, an unpacked record with mixed state domains, a fixed unpacked
  array, a packed record and a nested tagged union. Two-state members convert
  X/Z to zero on write (§7.2.1).
- `unpacked_contexts` passes unpacked tagged unions through value, output,
  inout, ref and const-ref formals, automatic and static function locals and
  results, ports, unpacked arrays (runtime index evaluated once) and a record
  member. Selected active members are actuals for formals of the member type.
- `member_selects` and `wrong_tag_selects` cover bit, part, indexed-part and
  element selects of tagged members. Active selects behave like ordinary
  selects; an inactive member's select is a run-time error that reads X and
  writes nothing, with its selector still evaluated once.
- `patterns` and `case_modes` cover tagged patterns over unpacked payloads,
  binding scope through `&&&` clauses, nested tags, and undefined (X) tags in
  `case`, `casex` and `if ... matches`.
- `capacity_boundary` and `oversized_payload_columns` straddle the packed
  value capacity: the largest legal finite payload keeps one owner; one bit
  more keeps the tag and each member in separate columns (RTL-101), so the
  default-filled `Table` reads `5a` at both ends and `tagged Empty` matches.
  The oversized array source of a whole-value binding is still rejected
  instead of being flattened (`neg_oversized_pattern`). Real, string and handle
  payloads keep separate member storage since SIM-007 (its `native_tagged`
  fixtures). `neg_binding_scope` and
  `neg_member_value` are the nearest language-illegal forms.

## Q03: member NBAs and retagging

A nonblocking assignment fixes its target and value when it is issued
(§§4.9.4, 10.4.2) and performs the assignment at commit. A member assignment
must be consistent with the tag current when it is performed, and an
inconsistent one is a run-time error (§11.9); a member assignment never
changes the tag, and a value of one member is never stored under another
member's tag (§7.3.2). Therefore:

- a wrong tag at issue is reported at issue and nothing is queued;
- a valid issue whose member is active again at commit (same-member retag,
  retag away and back, no retag) publishes the issue-time value
  (`q03_same_tag_witness`, `q03_stable`);
- a valid issue whose member is inactive at commit (another member, void, a
  narrower member, another nested tag, a whole-variable NBA committed first, a
  future NBA retagged before its time) reports
  `nonblocking write to tagged-union member M at <source> found an inactive
  tag at commit` and stores nothing (`q03_retag_witness`, `q03_retag_kinds`).

Leaving the retagged value unchanged after the report is the owner policy for
the post-error state; the clauses fix only that the access is an error.
`q03_race` races processes woken by one event. Every blocking retag in the
Active region precedes the NBA region, so the member write fails either at
issue or at commit; a racing whole-variable NBA either commits before the
member write or after it. Both orders end in the retagged value, and the
test accepts exactly that set of reports.
