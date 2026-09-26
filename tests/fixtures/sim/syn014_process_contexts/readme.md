# SYN-014 process contexts

`process_contexts.sv` is the positive SYN-014 witness. It combines fixed
unpacked records, packed members, nested array leaves, an aggregate input
link, a called helper with both conditional arms, disjoint packed writers,
an enabled latch, and a resettable flip-flop. The helper call count proves
that an `always_comb` process still wakes when its currently unselected
aggregate arm changes. The child output proves that an aggregate input link
publishes nested element changes to the child process. The flip-flop output
stays stable for a data-only update and changes only at its declared clock or
reset event.

`overlapping_writers.sv`, `extra_event.sv`, and `latch_event.sv` are
single-fault rejection controls for overlapping packed writers, a second
`always_ff` event control, and an explicit `always_latch` event control.

`repaired_expression_contexts.sv` requalifies the always family and `@*`
against expression kinds admitted by the fixed-value and pattern repairs:
unpacked-structure conditionals (both arms are dependencies; an unchanged
result does not notify a downstream reader), array-valued `inside`, structure
parameter equality, a tagged `matches` predicate with a later `&&&` clause,
positional-pattern lvalues fed by a helper call, memory selector/content reads,
a closed latch and a whole-record `always_ff` nonblocking write.
`disjoint_repaired_writers.sv` keeps distinct array elements and structure
members legal across `always_comb`/`always_ff`; `pattern_writer_overlap.sv`,
`member_ff_overlap.sv` and `pattern_continuous_overlap.sv` are the matching
single-fault overlap rejections.

The source forms are mapped to IEEE 1364-2001 §9.7.5 and §9.9.2 and
IEEE 1800-2009 §§7.2, 7.4.2, 7.4.6, 7.7, and 9.2.2.2–9.2.2.4 in
[`docs/specification/spec-reference-verilog.md`](../../../../docs/specification/spec-reference-verilog.md)
and [`docs/specification/spec-reference-sv.md`](../../../../docs/specification/spec-reference-sv.md).
The public suite runs the positive witness in both optimizer modes.
This is bounded execution evidence; dynamic/native aggregate callbacks,
testbench event objects, and concurrent assertion scheduling remain outside
SYN-014.
