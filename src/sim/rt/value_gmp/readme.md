# Compact packed values

This C11 backend uses value ABI 5 and implements storage/ownership,
core arithmetic and initialized arithmetic destinations, bitwise/logical operators, equality/relations, integral mux,
div/mod/pow/clog2, shifts/reductions, case modes, directional wildcard equality,
range membership, selections/captured plans, packed reference reads,
concatenation, replication, streaming, array conditional merge,
net/strength/UDP/enum, real/time and formatting/scalar/index adapters.
V06 consumer primitives are implemented on native A/B words in
`consumer_bridge.c`, with inline small scanner mutations in `consumer_inline.h`.
Every generated model uses this backend with GMP kernels; the legacy backend
and portable kernels remain development references.

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

`net_adapters.c` resolves full nets and unaligned ranges with word masks,
including both X strength endpoints, wired ties and implicit pull/supply sources.
UDP inputs normalize Z to X and rows match in declaration order. Enum navigation
clones declaration-ordered members (last duplicate wins) or the supplied default.
Strength and net metadata remain outside packed storage.

`real_time.c` keeps native doubles and scheduler ticks distinct from packed
values. Integer casts round half away from zero; rtoi truncates. Nonfinite
integer conversions yield X, while delay conversion rejects nonfinite/negative
real values and checked tick overflow. Bit conversions preserve IEEE payloads;
X/Z contributes zero on reads. Packed-to-real follows legacy's high-to-low limb
rounding without allocating a magnitude. Wide real-to-packed places the rounded
binary significand directly and wraps at the requested width.

`format_index.c` provides exact host index checks, word-parallel wide signed
representability, bounded formatting prefixes and independent decimal magnitude
scratch. Small decimal conversion allocates nothing. Portable decimal kernels
use nine-digit chunks; GMP `mpn_get_str` is used at/above the named
`LLG_SV4_DECIMAL_GMP_THRESHOLD` (four significant limbs). Neither path changes
borrowed input or exports GMP types. X wins over Z in radix groups; any X/Z
prints `x` in decimal.

Operations have separate translation units. `kernels.c` alone
includes GMP when `LLG_SV4_GMP_KERNELS=1`; portable mode has no GMP dependency.
GMP requires 64-bit nail-free limbs; 32-bit-limb and nail builds are rejected at
configure and compile time. Values store `uint64_t` words. GMP's 64-bit limb is
the same C type on LP64 Linux and 64-bit Windows (`_LONG_LONG_LIMB`), so those
arrays pass to `mpn` directly. macOS declares `uint64_t` as `unsigned long long`
while GMP's limb is `unsigned long`: same size, distinct type. Casting would
violate C aliasing rules, so the kernels then `memcpy` operands into native limb
scratch and copy results back. That costs one freed heap scratch per wide
mul/div/mod/decimal call (`llg_gmp_sv4_kernel_scratch_allocations()` reports it
for the allocation probes); portable kernels never pay it. Wide multiplication computes the
low half directly below `LLG_SV4_MUL_FULL_THRESHOLD` (128 words by default). At or
above the threshold GMP uses a full product in a temporary tail of the result
allocation, then shrinks it before publication. Portable mode always computes
the low half. Mixed add/sub allocate only their result. Division uses GMP's public
`mpn_tdiv_qr` or portable base-2^32 Knuth division; signed results truncate toward
zero, retain dividend remainder sign, and wrap at width. Pow uses repeated
truncated squaring; bases 1 and all-ones return from the exponent parity, and an
even base stops once its square reaches zero (at most log2(width) squarings), so
only other odd bases pay one product per exponent bit. There is no global/TLS workspace or allocator-hook policy.

[Native probes](../../../../tests/runtime_value_storage/readme.md) build both
kernels against live legacy and independent integer/state oracles. Generated
selection, packaging, frame layouts, caches and containing-owner integration
remain later work. Missing operations have no legacy conversion fallback.

S4/S5 uses `selections.c`, `references.c` and `assembly.c`, with shared private
word-range copies in `ranges.h` and inline <=64-bit paths in
`selection_inline.h`. Captured plans retain intermediate clipping and source
coordinates; reference graphs borrow stable cells and callback/graph storage.
Every result owns independently. Selected aliases snapshot before writes; known
selected intervals do not promote B even if another source interval contains X/Z.
Reversed intervals use fixed 64-bit bit permutations and shifted range copies.
Streams gather slices into destination words before storing each word once.
`sv4_stream`/`sv4_unstream` preserve width;
assignment padding/truncation remains the caller's conversion before/after them.
Array conditionals compare each immediate element with logical equality and use
the supplied default for differing or X/Z-containing elements (SV2009 11.4.11).

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


`sv4_checked_width`, `llg_real_to_bool`, `llg_ref_read`/`llg_ref_view_valid`
and source-compatible owner-free reference/selection types are implemented.
The [facade checklist](facade_audit.md) records operations, helpers and the
integrated source-level surface.

Consumer mutations preserve width, sign and owner identity without scratch
values. Known writes allocate nothing. Masked/range writes inspect only selected
source bits before promoting B; a canonical mutation removes an all-zero B once.
External VPI32 records use memcpy for alignment and foreign-type safety, clip
imports, preserve untouched halves and ignore padding X/Z. Copied wait snapshots
use native A/B export and comparison; waveform text loads each A/B word once.
The complete standalone build target is `compact_checks`; its consumer probes
include independent state/arithmetic oracles, differential checks and allocation
counters.

Generated builds use compact values with GMP kernels (development builds may
select portable kernels with `LLG_DEV_COMPACT_KERNELS=portable`); GMP builds the
bundled subset (below) unless `GMP_ROOT` names an installation. Exported CMake projects propagate both
literal definitions to every model/runtime unit; with an installation they also
verify matching GMP headers/library and 64-bit nail-free compatible limbs. The
facade selects native compact consumer and reference implementations;
there are no pending declarations for currently emitted operations and no legacy
fallbacks.

Selected compact units are embedded in this build order: storage, logic,
arithmetic, shifts/reductions, comparison/membership, selections, references,
assembly, consumer bridges, kernels, net adapters, real/time and formatting/index.
`llg_value.c` is a facade-only unit for compact; its reference implementation is
`value_gmp/references.c`. S2–S5 and consumer helpers use portable word loops with
either kernel selection.

EMIT-1 arithmetic destinations are implemented in `backend.h` and `arithmetic.c`.
Small results stay inline. Equal-width known add/sub reuse destination storage,
including exact aliases, without allocation. Independent equal-width multiply
writes through the existing limb kernel, using scratch only for a full GMP
product; known aliases compute a fresh owner before replacement. Unknown results
fill X after inspecting inputs, promoting B only when needed; known replacement
results remove B. Mismatched widths retain returning-operation extension rules.
Bitwise `_into` destinations (V10) overwrite a live equal-width destination in
place for known operands, exact aliases included; X/Z operands, other widths
and empty destinations take the returning form.

Net resolution starts with the A plane only and promotes B once, at the first
word carrying X/Z, so a known resolved net costs one allocation instead of an
allocation plus a shrinking reallocation.

## Bundled GMP, platform qualification and licensing

GMP kernels call `mpn_mul_n`, `mpn_mul_1`, `mpn_addmul_1`, `mpn_tdiv_qr` and
`mpn_get_str`. By default `llg` builds them from the `vendor/gmp` submodule
(GMP 6.3.0) without GMP's autotools: [`../gmp.rs`](../gmp.rs) embeds the
generic-C `mpn` subset forming their link closure, `gmp-h.in`, `gmp-impl.h`,
`longlong.h`, the licence texts and the tables generated for 64-bit nail-free
limbs ([`../gmp/generated`](../gmp/generated), regenerated and checked by
`scripts/gmp_tables.py`). Generated projects receive them under `gmp/`;
[`../gmp/llg_gmp.cmake`](../gmp/llg_gmp.cmake) configures `gmp.h` and a minimal
`config.h` for the target (64-bit, little-endian; `unsigned long long` limbs
where `unsigned long` is 32-bit, as on Windows; `static __inline` copies of
`gmp.h`'s inline functions under MSVC, whose plain `__inline` would define them
externally in every object) and compiles the OBJECT library `llg_gmp` into the
runtime archive. The archive is keyed by the bundle's content
hash, so GMP compiles once per runtime cache entry. Moving the submodule needs the
tables regenerated and the `LLG_GMP_MPN_SOURCES` closure rechecked:
`tests/runtime_gmp_closure.rs` links an unoptimized executable against every
`mpn`/`gmp` name the preprocessed sources contain, which also covers calls
that GCC/Clang fold away but MSVC `/Od` keeps. The
subset is portable C (no assembly `mpn` kernels), so an asm-tuned installation
can be faster for very wide operands.

`GMP_ROOT` (or `CmakeBuildOpts::gmp_root`) instead names an installation
(`include/gmp.h` plus a static `lib/libgmp.a`, `lib/gmp.lib` or `lib/libgmp.lib`).
Headers, library, compiler ABI and limb configuration must match the model
compiler; CMake checks version agreement and 64-bit nail-free limbs, and the
content hash keeps runtime archives of different installations apart. A
host-tuned build (for example `-march=native`) is valid only on matching CPUs.
Every CI test lane builds the bundled GMP with its target's compiler (MSVC
x64/arm64, macOS arm64, Linux musl and glibc distributions); installation tests
run when `LLG_TEST_GMP_ROOT` is set.

Licensing (review, not legal advice): GMP is dual-licensed LGPLv3-or-later /
GPLv2-or-later. Lapligence carries the GMP subset as source text: the `llg` and
`llg_ls` executables never link it, but every generated model compiles and
links it. Such a model also contains the GPLv2 Lapligence
runtime, so whoever distributes it relies on GMP's GPLv2 option (LGPLv3 alone is
not GPLv2-compatible) and must ship GMP's licence texts and corresponding source;
generated projects carry both under `gmp/`. Under LGPLv3 alone (a differently
licensed runtime), static linking would additionally require relinkable object
files. Only the legacy and portable development references build without GMP.
See `THIRD_PARTY_NOTICES.md`.
