# Fixed integral and record layouts

The task module runs every positive fixture through the public CLI with both HDL
optimizer modes. `nested_records` and `packed_boundaries` also use checked
compilation and validated owned emission after snapshot destruction, then execute
after Db destruction with native O0/O3 in both HDL modes.

| Fixture | Clause and independent oracle |
| --- | --- |
| `q01_member_conversion` | IEEE 1800-2009 §§6.11.2, 7.2.1: X/Z in two-state members reads as zero; four-state members preserve X. Adopted FND-002 Q01 witness, with quiet `$finish(0)`. |
| `nested_records` | §§6.19, 7.2.2, 7.4, 7.6: defaults are X/zero by member domain; enum -2 stays signed; copies and slices retain nested declaration order and independent storage. |
| `packed_boundaries` | §§7.2.1, 7.4.1: a 65-bit signed all-ones member is -1. Updating its low 64 bits preserves bit 64; clearing bits 66:63 clears the high four physical bits. Negative/nonzero indices map through their declared bounds; an invalid write has no effect and an invalid four-state read yields X. |
| `union_views` | §7.3: equal packed views overlap; writing low nibble 1 to FE gives F1 (-15). Project max-width unpacked integral overlay contract: ABCD read as an 8-bit member is CD; writing 12 preserves AB. This byte alignment is the project's representation choice, not a universal unpacked-union LRM representation. |
| `record_nets` | §§6.7, 7.2.2, 28.12: undriven members are Z, including members with declaration defaults (ignored for nets). 0F opposed by F0 resolves XX; releasing the former yields F0. Other leaves retain their own contributions. |
| `record_net_arrays` | §§6.7, 7.4: constant coordinates select distinct cells and recursive member-array lanes; source changes update only the selected contributions. |
| `packed_record_nets` | §§6.7, 7.2.1: independently driven packed members compose A5, then C5. |
| `record_port_composition` | §§6.7, 23.3.3.7: an inout record net retains child/parent electrical identity and signed -1 member reads. |
| `memory_elements_2001.v` | IEEE 1364-2001 §3.10: indexed memory elements are legal and preserve 5A/A5. Whole memory values remain a separate illegal boundary. |

The adopted FND-002 negatives are `neg_unequal_packed_union` (§7.3),
`neg_untagged_dynamic` (§7.3.2), and `neg_whole_array_2001.v` (V2001 §3.10).
`neg_packed_native` (§7.2.1) excludes string packed members;
`neg_record_net_state` (§6.7) excludes two-state net members.
`neg_unpacked_union_net` excludes unpacked unions recursively nested in record
net arrays: §6.7 lists four-state integral types and fixed arrays/unpacked
structures of valid net types. A packed union is integral; an unpacked union is
not in that list. Slang's broader admission does not change the 2009 boundary.

FND-002's `native_record_call` and string-bearing `record_slice` witnesses belong
to the later native/resizable slice (SIM-003); this task exercises their fixed
integral counterparts without promoting native support. Variable/union views
reuse existing lifecycle and facade operations, with no new runtime fragments,
allocators or suspension points. Allocation-failure and cancellation additions
are therefore inapplicable. The recursive-layout/validator and net-offset tests
are component checks, separate from generated-model execution.
