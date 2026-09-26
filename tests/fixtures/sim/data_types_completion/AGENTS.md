# Datatype completion fixture contracts

These seventeen completion-suite positives and the focused SYN-028 witnesses use the local IEEE 1800-2009 LRM. Reductions preserve
typed wide arithmetic, X/Z behavior, empty identities, and the specified associative order
independence; array callbacks retain their result width and item binding.

| Fixture | Contract | Local LRM basis |
| --- | --- | --- |
| `string_real_conversion.sv` | numeric-prefix, whitespace/sign/exponent, invalid-string `atoreal`, and nonempty exact-value `realtoa` round trips | §6.16.10, §6.16.15 |
| `string_wide_real_contexts.sv` | 128-/512-bit real/integral assignment and argument conversions above 64 bits | §6.12.2, §6.16.10, §6.16.15 |
| `packed_struct_assignment_patterns.sv` | 128-/512-bit positional, named, default, simple-type, and named-type declaration patterns, including mixed two-/four-state members and generate scope | §7.2.1, §10.9.2 |
| `packed_union_assignment_patterns.sv` | direct packed initialization, selected-member writes, two-state conversion, and shared 128-/512-bit representation | §7.3.1 |
| `unpacked_aggregate_assignment_patterns.sv` | unpacked struct declaration patterns and unpacked-union selected-member writes over fixed packed integral members | §7.2, §7.3, §10.9.2 |
| `recursive_unpacked_aggregates.sv` | nested fixed unpacked structs/arrays with packed, real, and string leaves, deep copy/member updates, and unequal-width untagged-union storage | §7.2, §7.3 |
| `recursive_assignment_patterns.sv` | nested typed assignment patterns over fixed unpacked structs/arrays; each distinct source expression is evaluated once before recursive writes | §7.2, §10.9.2 |
| `resizable_assignment_patterns.sv` | bounded dynamic-array, queue, and associative-array assignment-pattern initialization and updates | §5.10, §5.11, §7.5, §7.8, §7.10, §10.9 |
| `pattern_nested_default.sv` | nested struct and fixed-array defaults recurse through aggregate members and preserve explicit keyed values | §10.9.1, §10.9.2 |
| `pattern_side_effect_count.sv` | explicit assignment-pattern expressions are evaluated once while recursive defaults and type keys provide covered leaves | §10.9.1, §10.9.2 |
| `dynamic_array_reductions.sv` | element-typed 128-bit reductions, modular arithmetic, X/Z propagation, and empty identities | §7.12.3 |
| `queue_reductions.sv` | signed 512-bit reductions and empty identities | §7.12.3 |
| `associative_array_reductions.sv` | order-independent 128-bit reductions retaining high bits and empty identities | §7.12.3 |
| `array_methods.sv` | queue and integral-key associative locator/min/max/unique result order, sort/rsort/reverse/shuffle mutation, and typed `with` callback evaluation including `item.index()` | §7.12 |
| `syn_027_fixed_reverse.sv` | fixed-array `reverse()` for lengths 1/2/3/17, ascending/descending/negative bounds, byte and packed-record elements, automatic locals/formals, selected rows, and reverse-twice restoration | §7.12.2 |
| `syn_028_fixed_sort.sv` | fixed-array `sort()`/`rsort()` for signed, unsigned, enum, packed-record-map, repeated-key, ascending/descending-bound, singleton, and automatic-ref cases | §7.12.2 |
| `syn028_unpacked_record_maps.sv` | signed mapped ordering of unpacked records in a zero-time function and descending declared range; repeated keys preserve record membership without a stability requirement | §7.12.2 |
| `syn028_edition_boundary.sv` | fixed `sort()`/`rsort()` operate in 2009 and reject in 2001 with an otherwise 2001-valid array declaration | §7.12.2 |
| `reduction_with_unsupported.sv` | legal width-changing reduction `with` callback evaluates each item and retains the callback result type | §7.12.3 |

The eight negative contracts each contain one fault: `syn_027_reverse_with.sv` and
`syn_027_reverse_const_ref.sv` reject fixed-array reverse forms; `syn_028_sort_bad_map.sv` and
`syn_028_sort_const_ref.sv` reject fixed-array sort forms; the four assignment-pattern fixtures
reject duplicate, missing, out-of-range, or duplicate-key pattern entries.

Require exact range/state/signedness matching for packed leaves and packed-integral
typedef keys. The listed fixed nested cases cover copy/member updates with packed,
real and string leaves. Other tagged/class/virtual-interface, nominal-key,
recursive-default, resizable/object-member and aggregate port/net/subprogram forms
are outside this bounded contract; the listed positives do not qualify them.
Run all seventeen completion-suite positives and both focused 2009 witnesses in both modes. Recursive-fixture ASan/UBSan/leak
validation remains a separate gate, not implied by native execution.

Local specification anchors verified in `docs/specification/spec-reference-sv.md`: §§6.12.2,
6.16.10, 6.16.15, 7.2, 7.2.1, 7.3, 7.3.1, 7.12.2, 7.12.3, 10.9, and 10.9.2.
