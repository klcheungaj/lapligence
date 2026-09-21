# Datatype completion fixture contracts

These independently authored black-box fixtures were derived from the local IEEE 1800-2009 LRM
before `llg` execution. The seventeen positive contracts are listed below. Reductions preserve
typed wide arithmetic, X/Z behavior, empty identities, and the specified associative order
independence; array callbacks retain their result width and item binding.

| Fixture | Contract | Local LRM basis |
| --- | --- | --- |
| `string_real_conversion.sv` | Positive: numeric-prefix, whitespace/sign/exponent, invalid-string `atoreal`, and nonempty exact-value `realtoa` round trips | §6.16.10, §6.16.15 |
| `string_wide_real_contexts.sv` | Positive: 128-/512-bit real/integral assignment and argument conversions above 64 bits | §6.12.2, §6.16.10, §6.16.15 |
| `packed_struct_assignment_patterns.sv` | Positive: 128-/512-bit positional, named, default, simple-type, and named-type declaration patterns, including mixed two-/four-state members and generate scope | §7.2.1, §10.9.2 |
| `packed_union_assignment_patterns.sv` | Positive: direct packed initialization, selected-member writes, two-state conversion, and shared 128-/512-bit representation | §7.3.1 |
| `unpacked_aggregate_assignment_patterns.sv` | Positive: unpacked struct declaration patterns and unpacked-union selected-member writes over fixed packed integral members | §7.2, §7.3, §10.9.2 |
| `recursive_unpacked_aggregates.sv` | Positive: nested fixed unpacked structs/arrays with packed, real, and string leaves, deep copy/member updates, and unequal-width untagged-union storage | §7.2, §7.3 |
| `recursive_assignment_patterns.sv` | Positive: nested typed assignment patterns over fixed unpacked structs/arrays; each distinct source expression is evaluated once before recursive writes | §7.2, §10.9.2 |
| `resizable_assignment_patterns.sv` | Positive: bounded dynamic-array, queue, and associative-array assignment-pattern initialization and updates | §5.10, §5.11, §7.5, §7.8, §7.10, §10.9 |
| `pattern_nested_default.sv` | Positive: nested struct and fixed-array defaults recurse through aggregate members and preserve explicit keyed values | §10.9.1, §10.9.2 |
| `pattern_side_effect_count.sv` | Positive: explicit assignment-pattern expressions are evaluated once while recursive defaults and type keys provide covered leaves | §10.9.1, §10.9.2 |
| `dynamic_array_reductions.sv` | Positive: element-typed 128-bit reductions, modular arithmetic, X/Z propagation, and empty identities | §7.12.3 |
| `queue_reductions.sv` | Positive: signed 512-bit reductions and empty identities | §7.12.3 |
| `associative_array_reductions.sv` | Positive: order-independent 128-bit reductions retaining high bits and empty identities | §7.12.3 |
| `array_methods.sv` | Positive: queue and integral-key associative locator/min/max/unique result order, sort/rsort/reverse/shuffle mutation, and typed `with` callback evaluation including `item.index()` | §7.12 |
| `syn_027_fixed_reverse.sv` | Positive: fixed-array `reverse()` for lengths 1/2/3/17, ascending/descending/negative bounds, byte and packed-record elements, automatic locals/formals, selected rows, and reverse-twice restoration | §7.12.2 |
| `syn_028_fixed_sort.sv` | Positive: fixed-array `sort()`/`rsort()` for signed, unsigned, enum, packed-record-map, repeated-key, ascending/descending-bound, singleton, and automatic-ref cases | §7.12.2 |
| `reduction_with_unsupported.sv` | Positive: legal width-changing reduction `with` callback evaluates each item and retains the callback result type | §7.12.3 |

The eight negative contracts each contain one fault: `syn_027_reverse_with.sv` and
`syn_027_reverse_const_ref.sv` reject fixed-array reverse forms; `syn_028_sort_bad_map.sv` and
`syn_028_sort_const_ref.sv` reject fixed-array sort forms; the four assignment-pattern fixtures
reject duplicate, missing, out-of-range, or duplicate-key pattern entries.

Tagged unions, classes, virtual interfaces, nominal type keys, recursive defaults,
resizable/object members, and aggregate ports/nets/subprogram storage remain outside the support
contract. Root-run normal validation covers all seventeen positive cases in both optimization modes;
ASan/UBSan/leak validation for the new recursive fixture remains a separate gate. This is
bounded evidence, not exhaustive conformance.

The supported aggregate contracts require exact range, state, and signedness matching for packed
leaves and use packed-integral typedef keys. Fixed nested aggregate paths additionally cover
recursive value copy and member updates for packed, real, and string leaves. These contracts do
not establish support for nominal type keys or the other excluded recursive/default/object and
aggregate storage forms.

Local specification anchors verified in `docs/specification/spec-reference-sv.md`: §§6.12.2,
6.16.10, 6.16.15, 7.2, 7.2.1, 7.3, 7.3.1, 7.12.2, 7.12.3, 10.9, and 10.9.2.
