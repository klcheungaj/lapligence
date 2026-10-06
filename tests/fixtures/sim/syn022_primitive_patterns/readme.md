# SYN-022 primitive conditional patterns

`primitive_runtime.sv` qualifies integral constant, wildcard, and identifier
patterns in `if`, `?:`, and ordered `&&&` predicates. A function supplies runtime
values so both optimizer modes exercise source capture, short circuiting, signed
7-bit, 33-bit, and 129-bit comparisons, one-bit two-state conversion, enum
constants, X/Z exact matching, and lexical shadowing. The
expected checks and calls follow IEEE 1800-2009 §§12.6, 12.6.2–12.6.3.

The two `bad_binding_*` files each use one binding outside its permitted true-arm
scope. `edition_boundary.sv` isolates `matches` as its only SystemVerilog syntax:
it runs in SV2009 and is rejected in Verilog-2001. Whole aggregate and tagged
patterns remain in the N08, SYN-023, and SYN-024 suites. The composite
`bad_duplicate_binding.sv` is a rejection-only control for the §12.6 rule that
identifiers be unique within one pattern; successful structure matching remains
with SYN-023. Dynamic-array wildcard/binding are legal syntax outside the
supported fixed-value execution profile and must receive specific errors.
`real_binding.sv` and `string_binding.sv` (SIM-007) bind whole real and string
values: the first filter passes (1.5 > 1.0; length 3) and the arm reads the
binding (1.50, doubled 3.00; `abc`, joined `abc!`); the second filter fails
(1.5 < 1.0; `abc` == `xyz`) and takes the else arm.

| Selected cell | Witness |
| --- | --- |
| Primitive type/value pairs | This fixture: signed 7-bit logic, unsigned 33/129-bit logic, one-bit two-state `bit`, enum, known/X/Z constants. `sequential_predicates/syn_022_basic_patterns.sv` retains unsigned 8-bit, function, and `always_comb` controls. |
| `if`, `?:`, ordered `&&&`, effects and scope | Both positive fixtures exercise all three forms; this fixture checks source counts, false/ambiguous prefixes, later-clause binding, true-arm access and outer-name shadowing. The two scope negatives reject else and post-if access. |
| Owned path and neighboring Boolean predicates | `slang_semantics::sequential_predicates` checks owned pattern kinds, automatic declaration identities, and lowering after snapshot teardown. `sim_sequential_predicates` retains the R06 truth table and dependency controls. |
| N08 whole fixed values | `review_bundle/n08_whole_patterns.sv` checks packed record, fixed unpacked record/array and tagged whole bindings, later filters, source snapshots, and false-prefix suppression. `review_bundle/n08_binding_scope_error.sv` rejects false-arm access. |
| Invalid or unselected forms | The duplicate-name control checks the §12.6 uniqueness rule. Dynamic-array wildcard/binding controls must reject; real and string bindings run; `edition_boundary.sv` runs in SV2009 and rejects in Verilog-2001. |

Mixed two-state/four-state **structure** constant patterns are Q01 under SYN-023;
these primitive integral comparisons do not establish that separate oracle.
