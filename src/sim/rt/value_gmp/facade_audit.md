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
| EMIT-1 `sv4_add_into`, `sv4_sub_into`, `sv4_mul_into` | Inline small paths in `backend.h`, matching-width reuse in `arithmetic.c`; exact aliases, X/Z fill, sign/width parity and canonical B transitions implemented. |
| V10 `sv4_and_into`, `sv4_or_into`, `sv4_xor_into`, `sv4_xnor_into` | Inline small paths in `backend.h`; known equal-width live-destination reuse in `arithmetic.c`; other cases use the returning form. |
| `sv4_<op>_to` destination forms (`value/destinations.h`) | Backend-neutral `value/destinations.c`, appended to the compact `llg_value.c`; compact link names are prefixed `llg_gmp_sv4_<op>_to` with public aliases. |
| Shifts, reductions, countones/onehot, wildcard/case modes, membership, logical implication/equivalence | `backend.h`, `shifts_reductions.c`, `comparison_membership.c`; inline small paths and direct word kernels |
| Selection functions and plans | `selection_inline.h`, `selections.c`; clipped coordinate plans, alias snapshots and word range copies |
| Stream/unstream, concat/repeat, array conditional | `selection_inline.h`, `assembly.c`; independent unsigned owners |
| Enum, full/range resolution, strength resolution, UDP | `net_adapters.c`; word-parallel endpoints, no per-bit resolution |
| Real/shortreal bits, integer/real conversions, checked delay scaling | Header and `real_time.c` |
| Formatting, decimal, low i64, exact i64/index checks | Header, `format_index.c` and decimal kernel seam |
| `sv4_checked_width` | Header; rejects unknown/negative/over-capacity input, preserves zero sentinel |
| `llg_sv4_word_t`, `llg_sv4_vpi_word_t`, V01 `llg_sv4_*` bridge functions and `LLG_SV4_BITS/X/Z` | G1 header and storage unit; copied logical words, no native/GMP views |
| V06 masked/range access, mask maintenance, in-place known arithmetic/two-state/digit operations, VPI32 and text export | `consumer_bridge.c`, `consumer_inline.h` and header aliases; native A/B loops, selected X/Z promotion and canonical B removal. All V06 consumer additions are embedded in selected generated builds. |
| All seven `LLG_RESOLVE_*` and eight `LLG_STRENGTH_*` constants | Prefixed enum constants and public aliases, identical values |
| `llg_real_to_bool` | Inline scalar helper; nonzero including NaN is true, signed zeros false |
| `sv4_select_plan_t` | Owner-free five-field equivalent in `reference_types.h`, guarded by `LLG_GMP_SV4_SELECT_PLAN_DEFINED`; shared by S4 and S9 |
| `llg_queue_t`, queue read/write callback types | Same opaque queue tag; callback packed arguments/results use the selected owner |
| `llg_ref_kind_t`, `llg_ref_t`, `llg_ref_composite_t`, `llg_ref_tag_check_t`, `llg_ref_view_t`, all ten `LLG_REF_*` constants | Source-compatible owner-free equivalents and aliases in `reference_types.h` / `backend.h` |
| `llg_ref_read`, `llg_ref_view_valid` | `references.c`; direct compact implementations with standalone public aliases, all reference kinds and tag checks covered. V07 owns runtime publication/retention. |

The reference descriptors contain borrowed cell/callback/plan addresses; copying
a descriptor does not transfer or extend any lifetime. Their containing runtime
owns retention, publication and teardown. S4 and S9 use the single guarded plan
type in `reference_types.h`. Selected builds embed these implementations. Re-audit APIs added by intervening
feature merges, including descriptor-backed fixed arrays. The V06 closure includes
`llg_sv4_plane_slice`, masked NBA merge, clipped compare/copy, range compare/copy/
fill, mask removal/top, known modular arithmetic, append-digit/two-state mutation,
foreign VPI32 import/export, copied A/B snapshot comparison and waveform text.
Inline waits already use the V01 copied A/B export; they retain no payload view.

`compact_facade_adapters.c` builds the same client against live legacy, portable
compact and GMP compact. It executes every constructor macro, width boundary,
scalar helper, reference/selection field and callback signature. The independent
Python oracle covers signed/unsigned scalar conversion, invalid indices, bounded
text prefixes, exact real-to-integer rounding/truncation and bit reinterpretation.
Packed-to-real additionally preserves the legacy limb rounding order; its
mathematical oracle admits at most one double ULP for finite large integers.
Native component evidence supplements the separate generated-model parity tests.

The S2/S3 family probes exhaust four-state inputs through four bits, including
unequal widths and signs, and inclusive range triples with all sign combinations.
Independent Python state/integer vectors and live-legacy differential checks cover
wide and maximum widths, unknown/high shift counts, canonical B removal and
independent owners in both kernel configurations. `compact_v05a_checks` builds
these probes alongside the G1 core checks.

The V06 consumer probes exhaust masks/payload pairs through three bits, four-state
range fills and digit appends through four bits, and known arithmetic through
eight bits. Per-state and binary arithmetic oracles accompany live-legacy
comparisons over every G1 width and the exclusive limit minus one. Unaligned
foreign records, odd import counts, top padding, zero extension, exact aliases
and unchanged sign/shape are covered. ELF counters check no scratch allocations,
known writes from unknown sources without promotion, and one realloc for each
required B promotion/removal. `compact_checks` builds all S1–S9 and consumer
executables, including the S4/S5 targets.

The V05d audit compares the live value header at `fbc2aa36` with G1 merge
`ca2d4227`: the only added callable value APIs are EMIT-1 arithmetic destinations
and the already implemented V06 consumer primitives. All 152 current callable
`sv4_*`, `llg_sv4_*`, reference and real helper names have compact public aliases;
this inventory accompanies semantic tests, rather than replacing them. RTL-002
adds descriptor-backed array operations in the container runtime, built on
public value operations (`sv4_cast`, two-state conversion, move, clone and
comparison). Selected builds compile them against the chosen backend rather
than duplicating compact numeric operations. No pending declarations remain.

`compact_destinations.cmake` adds exhaustive four-state <=4-bit mixed-width/sign
oracles, live-legacy differential checks, initialized/empty/mismatched destinations,
left/right/both aliases, surviving independent inputs/results, wide padding and
canonical storage, and both sides of the full-product threshold. ELF allocation
counters check allocation-free small and equal-width known add/sub, independent
multiply reuse, alias result allocation and B promotion/removal. Benchmarks cover
all three destination operations at 1/64/65/256/4096 bits, known/X/Z and each exact
alias mode. These are standalone component witnesses; selected generated models
reach the same implementations through the `sv4_*_into` facade aliases.

The V08 audit covers retained owners merged through the stack-bounding work.
Packed `_to` destination forms (`value/destinations.c`) and pointer-passed
results/operands compile against the selected backend; recursive subprograms on
the chain arena keep packed locals in arena frames whose estimates follow the
selected descriptor. Runtime C outside the backends uses only facade names; numeric
`sizeof(sv4_t)` appears only as native allocation arithmetic. Every emitter render
context now carries the selected backend: assertion predicates, sampled
domains, sequence callbacks and storage initialization no
longer assume legacy layout (their C-stack frames did not use it, so generated
code is unchanged). The native runtime/scheduler/container/VPI/waveform probes
run against compact portable and GMP kernels as well as legacy (see the
[runtime probe guide](../../../../tests/runtime_value_storage/readme.md#selected-value-backend));
they access values only through the neutral bridge. `sv4_gmp_integration`
fixtures and the stack-bounds fixtures extend the HDL parity matrix.
