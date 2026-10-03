# Compact packed values

This standalone C11 backend reserves value ABI 5 and implements storage/ownership,
core arithmetic, bitwise/logical operators, equality/relations, integral mux,
div/mod/pow/clog2, shifts/reductions, case modes, directional wildcard equality,
and range membership. Production models still use the legacy backend.

`backend.h` supplies inline operations for widths through 64 and a static
`LLG_GMP_SV4_LITERAL(bits,x,z,width,sign)` initializer for those widths. Define
`LLG_SV4_GMP_PUBLIC_NAMES` to use the unchanged implemented `sv4_*` and neutral
`llg_sv4_*` names in standalone clients. There is no runtime backend dispatch.
By-value inputs borrow; returned values are independent owners. Initialize cells
with `SV4_EMPTY`, and release every returned owner.

Wide values own an exact-width A plane and a contiguous B plane only while X/Z
exists. Writes/results automatically drop zero B. All words are 64-bit with zero
padding; A/B uses 0=00, 1=10, X=11, Z=01. Bulk copied VPI words expose this logical
encoding without exporting private pointers. Constructors canonicalize old-plane
input, including X priority on overlap.

Storage, logic, arithmetic, shifts/reductions and comparison/membership are
separate translation units. `kernels.c` alone
includes GMP when `LLG_SV4_GMP_KERNELS=1`; portable mode has no GMP dependency.
GMP requires compatible 64-bit nail-free limbs. Wide multiplication computes the
low half directly below `LLG_SV4_MUL_FULL_THRESHOLD` (128 words by default). At or
above the threshold GMP uses a full product in a temporary tail of the result
allocation, then shrinks it before publication. Portable mode always computes
the low half. Mixed add/sub allocate only their result. Division uses GMP's public
`mpn_tdiv_qr` or portable base-2^32 Knuth division; signed results truncate toward
zero, retain dividend remainder sign, and wrap at width. Pow uses repeated
truncated squaring. There is no global/TLS workspace or allocator-hook policy.

[Native probes](../../../../tests/runtime_value_storage/readme.md) build both
kernels against live legacy and independent integer/state oracles. Generated
selection, packaging, frame layouts, caches and containing-owner integration
remain later work. Missing operations have no legacy conversion fallback.

Shifts treat counts as unsigned, return X for any unknown count, and check all
count words before narrowing. Arithmetic right shift repeats the sign-bit state,
including X/Z; every result keeps the left operand's width/sign. Reductions use
controlling known bits and otherwise propagate X; countones/onehot ignore X/Z.
Casez/casex helpers zero-extend their already-normalized caller inputs. Wildcard
equality ignores X/Z only on the right, extending signs only when both operands
are signed. Range membership combines inclusive pairwise comparisons with
four-state logical AND; frontend expression sizing stays with the caller.

Known equal-width wide add/sub and bitwise operations write an uninitialized
result allocation directly from the two planes. The inline facade passes private
plane pointers to this kernel, avoiding descriptor copies and the normalization
path. All words are written and padding masked before publication. Equality uses
a direct memcmp for equal-width known inputs; clone allocates without zeroing
before memcpy. Wide constant-state fills also write and mask their allocation
directly; nonempty X/Z fills need no zero-B scan. The inline arithmetic facade
returns these all-X results before copying descriptors to the generic kernel.
Copy reuse already uses a
direct memcpy.

Remaining public `sv4_*` operations at this revision:

`sv4_array_conditional_merge`, `sv4_bit_select`, `sv4_bit_select_set`, `sv4_bitstoreal`, `sv4_bitstoshortreal`, `sv4_checked_width`, `sv4_concat`, `sv4_delay_ticks`, `sv4_enum_navigate`, `sv4_fits_i64`, `sv4_format`, `sv4_from_real`, `sv4_idx_part_select`, `sv4_idx_part_select_set`, `sv4_idx_part_select_set_value`, `sv4_idx_part_select_value`, `sv4_part_select`, `sv4_part_select_set`, `sv4_real_delay_ticks`, `sv4_realtobits`, `sv4_repeat`, `sv4_repeat_count`, `sv4_resolve`, `sv4_resolve_strengths`, `sv4_resolve_strengths_range`, `sv4_rtoi`, `sv4_select_plan_bit`, `sv4_select_plan_indexed`, `sv4_select_plan_init`, `sv4_select_plan_part`, `sv4_select_plan_read`, `sv4_select_plan_set`, `sv4_select_plan_slice`, `sv4_select_plan_step`, `sv4_shortrealtobits`, `sv4_stream`, `sv4_to_dec_string`, `sv4_to_i64`, `sv4_to_index`, `sv4_to_index_i64`, `sv4_to_real`, `sv4_udp_eval`, `sv4_unstream`.

The packed-dependent `llg_ref_view_valid` and `llg_ref_read`, the shared scalar
`llg_real_to_bool`, and owner-free reference/selection types also await V05/V07.
