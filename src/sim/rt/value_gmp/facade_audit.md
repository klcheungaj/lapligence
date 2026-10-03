# Compact facade integration checklist

The standalone header uses prefixed symbols beside the live legacy oracle;
`LLG_SV4_GMP_PUBLIC_NAMES` exposes the implemented source-compatible names.
Generated selection and embedding use `ValueConfig` and the compile-time facade. Numerical operations never
convert into legacy descriptors as a fallback.

| Public surface in `llg_value.h` | Compact equivalent / integration owner |
| --- | --- |
| `sv4_t`, ABI and exclusive width limit | `backend.h`; compact ABI 5, exact-width owners |
| `SV4_EMPTY`, `SV4_INIT`, `SV4_C`, `SV4_S`, `SV4_X`, `SV4_Z`, `LLG_MASK` | Public aliases in `backend.h`; runtime constructors remain source-compatible. Additional `SV4_LITERAL` supports static initialization at <=64 bits. |
| Storage, constructors, resize/cast, two-state, arithmetic/logic/equality, relations, div/mod/pow/clog2 | G1 header and storage/logic/arithmetic/kernel units |
| Shifts, reductions, countones/onehot, wildcard/case modes, membership, logical implication/equivalence | Implemented V05/S2–S3 header fast paths, `shifts_reductions.c` and `comparison_membership.c`; unsigned full-width counts, X/Z sign fill, controlling-bit reductions, right-only wildcards and inclusive pairwise range comparisons |
| Selection functions and plans, stream/unstream, concat/repeat, array conditional | Pending V05/S4–S5 |
| Enum, full/range resolution, strength resolution, UDP | `net_adapters.c`; word-parallel endpoints, no per-bit resolution |
| Real/shortreal bits, integer/real conversions, checked delay scaling | Header and `real_time.c` |
| Formatting, decimal, low i64, exact i64/index checks | Header, `format_index.c` and decimal kernel seam |
| `sv4_checked_width` | Header; rejects unknown/negative/over-capacity input, preserves zero sentinel |
| `llg_sv4_word_t`, `llg_sv4_vpi_word_t`, V01 `llg_sv4_*` bridge functions and `LLG_SV4_BITS/X/Z` | G1 header and storage unit; copied logical words, no native/GMP views. The selected facade integrates V06 consumer primitives through the neutral and compact word bridges. |
| All seven `LLG_RESOLVE_*` and eight `LLG_STRENGTH_*` constants | Prefixed enum constants and public aliases, identical values |
| `llg_real_to_bool` | Inline scalar helper; nonzero including NaN is true, signed zeros false |
| `sv4_select_plan_t` | Owner-free five-field equivalent in `reference_types.h`, guarded by `LLG_GMP_SV4_SELECT_PLAN_DEFINED` for S4 integration |
| `llg_queue_t`, queue read/write callback types | Same opaque queue tag; callback packed arguments/results use the selected owner |
| `llg_ref_kind_t`, `llg_ref_t`, `llg_ref_composite_t`, `llg_ref_tag_check_t`, `llg_ref_view_t`, all ten `LLG_REF_*` constants | Source-compatible owner-free equivalents and aliases in `reference_types.h` / `backend.h` |
| `llg_ref_read`, `llg_ref_view_valid` | Shared algorithms in `value/references.c`, assembled as selected `llg_value.c`; compact symbols are prefixed. S4 dependencies remain unresolved until that family merges. |

The reference descriptors contain borrowed cell/callback/plan addresses; copying
a descriptor does not transfer or extend any lifetime. Their containing runtime
owns retention, publication and teardown. S4 should use the guarded plan type,
rather than introduce a second incompatible typedef. Re-audit APIs added by
intervening feature merges, including net-array extensions.

`compact_facade_adapters.c` builds the same client against live legacy, portable
compact and GMP compact. It executes every constructor macro, width boundary,
scalar helper, reference/selection field and callback signature. The independent
Python oracle covers signed/unsigned scalar conversion, invalid indices, bounded
text prefixes, exact real-to-integer rounding/truncation and bit reinterpretation.
Packed-to-real additionally preserves the legacy limb rounding order; its
mathematical oracle admits at most one double ULP for finite large integers.
Native component evidence does not establish generated model/backend selection.

The S2/S3 family probes exhaust four-state inputs through four bits, including
unequal widths and signs, and inclusive range triples with all sign combinations.
Independent Python state/integer vectors and live-legacy differential checks cover
wide and maximum widths, unknown/high shift counts, canonical B removal and
independent owners in both kernel configurations. `compact_v05a_checks` builds
these probes alongside the G1 core checks.
