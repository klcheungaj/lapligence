# Compact facade integration checklist

The standalone header uses prefixed symbols beside the live legacy oracle;
`LLG_SV4_GMP_PUBLIC_NAMES` exposes the implemented source-compatible names.
Generated selection and embedding remain V07 work. Numerical operations never
convert into legacy descriptors as a fallback.

| Public surface in `llg_value.h` | Compact equivalent / integration owner |
| --- | --- |
| `sv4_t`, ABI and exclusive width limit | `backend.h`; compact ABI 5, exact-width owners |
| `SV4_EMPTY`, `SV4_INIT`, `SV4_C`, `SV4_S`, `SV4_X`, `SV4_Z`, `LLG_MASK` | Public aliases in `backend.h`; runtime constructors remain source-compatible. Additional `SV4_LITERAL` supports static initialization at <=64 bits. |
| Storage, constructors, resize/cast, two-state, arithmetic/logic/equality, relations, div/mod/pow/clog2 | G1 header and storage/logic/arithmetic/kernel units |
| Shifts, reductions, countones/onehot, wildcard/case modes, membership, logical implication/equivalence | `backend.h`, `shifts_reductions.c`, `comparison_membership.c`; inline small paths and direct word kernels |
| Selection functions and plans | `selection_inline.h`, `selections.c`; clipped coordinate plans, alias snapshots and word range copies |
| Stream/unstream, concat/repeat, array conditional | `selection_inline.h`, `assembly.c`; independent unsigned owners |
| Enum, full/range resolution, strength resolution, UDP | `net_adapters.c`; word-parallel endpoints, no per-bit resolution |
| Real/shortreal bits, integer/real conversions, checked delay scaling | Header and `real_time.c` |
| Formatting, decimal, low i64, exact i64/index checks | Header, `format_index.c` and decimal kernel seam |
| `sv4_checked_width` | Header; rejects unknown/negative/over-capacity input, preserves zero sentinel |
| `llg_sv4_word_t`, `llg_sv4_vpi_word_t`, all `llg_sv4_*` bridge functions and `LLG_SV4_BITS/X/Z` | G1 header and storage unit; copied logical words, no native/GMP views |
| V06 consumer bridge primitives | Legacy helpers in `llg_value.h`; compact counterparts and generated consumer integration remain V07 work. |
| All seven `LLG_RESOLVE_*` and eight `LLG_STRENGTH_*` constants | Prefixed enum constants and public aliases, identical values |
| `llg_real_to_bool` | Inline scalar helper; nonzero including NaN is true, signed zeros false |
| `sv4_select_plan_t` | Owner-free five-field equivalent in `reference_types.h`, guarded by `LLG_GMP_SV4_SELECT_PLAN_DEFINED`; shared by S4 and S9 |
| `llg_queue_t`, queue read/write callback types | Same opaque queue tag; callback packed arguments/results use the selected owner |
| `llg_ref_kind_t`, `llg_ref_t`, `llg_ref_composite_t`, `llg_ref_tag_check_t`, `llg_ref_view_t`, all ten `LLG_REF_*` constants | Source-compatible owner-free equivalents and aliases in `reference_types.h` / `backend.h` |
| `llg_ref_read`, `llg_ref_view_valid` | `references.c`; direct compact implementations with standalone public aliases, all reference kinds and tag checks covered. V07 owns runtime publication/retention. |

The reference descriptors contain borrowed cell/callback/plan addresses; copying
a descriptor does not transfer or extend any lifetime. Their containing runtime
owns retention, publication and teardown. S4 and S9 use the single guarded plan type in `reference_types.h`. V07 must re-audit any APIs
added by intervening feature merges, including net-array extensions.

`compact_facade_adapters.c` builds the same client against live legacy, portable
compact and GMP compact. It executes every constructor macro, width boundary,
scalar helper, reference/selection field and callback signature. The independent
Python oracle covers signed/unsigned scalar conversion, invalid indices, bounded
text prefixes, exact real-to-integer rounding/truncation and bit reinterpretation.
Packed-to-real additionally preserves the legacy limb rounding order; its
mathematical oracle admits at most one double ULP for finite large integers.
Native component evidence does not establish generated model/backend selection.
