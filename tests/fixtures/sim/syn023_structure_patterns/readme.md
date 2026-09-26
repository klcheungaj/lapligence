# SYN-023 recursive fixed structure patterns

`recursive_runtime.sv` exercises whole packed and fixed unpacked record
wildcards/bindings, source snapshots, named and positional recursive member
patterns, omitted members, nested wildcards/bindings/constants, and a fixed
array-of-struct member binding. Filters and true arms read the bound value;
false prefixes do not evaluate a later source. `mixed_state_constants.sv`
isolates Q01. Both run through the public CLI in optimized and unoptimized
SV2009 modes. Expected output follows IEEE 1800-2009 §§12.6, 12.6.2–12.6.3.

**Q01 oracle:** IEEE 1800-2009 §7.2.1 says a packed structure with any
four-state member is four-state as a whole, but reading one of its two-state
members implicitly converts that member to two-state. Section 12.6 matches
each structure pattern against its corresponding member value. Thus a whole
record set to `'x` has a `bit [3:0]` member that matches `4'h0`, while its
`logic [3:0]` member matches `4'hx` under exact `if ... matches` comparison.
The opposite constant comparisons must fail. This is a member-read rule, not
a claim about tagged-union or wildcard case comparison modes.
**Disposition:** Q01 is a confirmed selected-path defect, repaired in the
recursive pattern member projection. Before the repair, the public fixture's
two-state zero constant check failed because the emitter compared a raw X
slice; after the repair, both optimizer modes pass all four checks.

The rejection-only fixtures each isolate one fault: duplicate identifiers
across nested members, incomplete positional shape, nested structure syntax
on an integral field, and an else-arm binding reference. `edition_boundary.sv`
contains only Verilog-2001-compatible syntax except `matches`; SV2009 runs it
and Verilog-2001 rejects it. Dynamic/native subjects and tagged payloads are
covered by the SYN-022 and SYN-024 boundaries, respectively.

| Selected cell | Witness |
| --- | --- |
| Whole record wildcard/binding and snapshot | `recursive_runtime.sv`: packed and fixed unpacked records, source function counts, mutation after match, `if` and `?:`. |
| Recursive structure form and scope | `recursive_runtime.sv`: named reorder/omission, positional nesting, nested wildcard and binding, later `&&&` filters, true-arm access, false-prefix suppression. |
| Nested fixed array of structs | `recursive_runtime.sv`: whole array member binding and indexed member reads. The §12.6 grammar has no array pattern node for element-by-element pattern syntax. |
| State-domain, signedness and width | `recursive_runtime.sv`: signed two-state count, four-state code and a 129-bit member spanning three limbs; `mixed_state_constants.sv`: exact X and converted two-state member constant checks. |
| Negative and edition boundaries | `bad_*` files and `edition_boundary.sv`; each is run with both optimizer modes. |
| Neighboring controls | `sim_sequential_predicates`, `sim_syn022_primitive_patterns`, and `sim_review_bundle_patterns` retain predicate, primitive, and N08 whole-value acceptance. |
