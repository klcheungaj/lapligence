# Packed-value facade contract

Feature code includes `llg_value.h` and uses `sv4_t`, the unchanged operation
signatures and the neutral bridge below. The [ownership guide](ownership.md)
owns scheduler/container release boundaries. Descriptor fields are private by
contract, even though C needs their definition to embed addressable cells.

## Production layout and staged selection

The legacy implementation remains in `value/` and remains selectable. Its
independent, immutable regression reference is `prototypes/sv4_gmp/golden/`;
never regenerate that snapshot from production. Production header drift is
reported separately by the golden verifier.

The production GMP implementation will live in `src/sim/rt/value_gmp/`.
V02-V05 populate storage, ownership, operations and conversions there. V07
splits the legacy definition/declarations into `value/backend.h` and makes
`llg_value.h` include **exactly one** of `value/backend.h` and
`value_gmp/backend.h`, followed by the selected neutral bridge. Shared
owner-free reference/selection types, callbacks, enums and constants remain
available through the facade with the same source signatures.

`LLG_SV4_USE_GMP` accepts the literal tokens `0` and `1`, defaults to `0`,
and rejects every other token. Public operation aliases are preprocessor
aliases, not wrappers: legacy `sv4_add` calls the existing `sv4_add` symbol;
production GMP `sv4_add` calls `llg_gmp_sv4_add`. Apply the same prefix to all
GMP `sv4_*` operations and packed-dependent `llg_ref_*` helpers; the pure
scalar `llg_real_to_bool` helper may remain shared. Missing GMP operations
must have no legacy declaration/alias or fallback. Backend symbol sets must
remain disjoint. A client linked to only the wrong library fails to link.
Never intentionally link both backend libraries into one generated model.

V01 adds the legacy bridge without splitting the embedded header or changing
runtime packaging. The production header accepts `0` and explicitly rejects
`1` as unavailable, rather than silently selecting legacy. Both selector modes
are exercised in the prototype facade: its GMP symbols retain the experimental
`gmp4_*` prefix. This is an executable dispatch witness, not production GMP
integration. Its 37 mapped operations do not establish coverage of the entire
109-operation production API or the three `llg_*` helpers. The declaration,
macro and type audit is `prototypes/sv4_gmp/tools/generate_api_coverage.py`.

Legacy production retains value ABI **4**. Reserve value ABI **5** for the
production GMP descriptor; prototype ABI **1** is experimental. V07 owns
embedding, CMake, backend/ABI/limb cache keys, frame layout, foreign-module
fences and all-translation-unit definitions. Configuration alone is not ABI
support. Regenerate model and runtime together; never cast a descriptor across
backends. There is no per-value backend tag, virtual/function table, TLS/global
scratch, backend branch inside an operation, or operation-level fallback.
Inline/wide and B-present checks are representation checks within GMP.

## Width, sign and language behavior

The frontend and IR determine context widths and signedness. A value records
its actual width below the exclusive `LLG_SUPPORTED_WIDTH_LIMIT` (`1 << 20`).
Width zero is an allocation-free internal empty value; constructors and
resize/cast accept it. This does not admit a zero-width HDL declaration;
declaration/selection shape validation remains the frontend or owning API's job.
Unused high bits of the final word are zero. Sign is boolean metadata; a
nonzero requested sign means signed. The bridge setter normalizes it to 0/1.

* `sv4_resize(v, w, s)` narrows by truncating high bits; widening extends the
  source MSB state when the **requested** `s` is nonzero, otherwise with zero.
  This applies even to an unsigned source. The result carries `s`.
* `sv4_cast(v, w, s)` truncates similarly, but widening extends the source MSB
  state when the **source** is signed, otherwise with zero. The result carries
  the requested `s`, independently of extension. An unsigned `8'h80` cast to
  signed 65 bits is positive 128; requested-sign resize to signed 65 bits
  extends ones. A signed `8'h80` cast to unsigned 65 bits extends ones.
* X and Z sign bits extend as X and Z respectively. Equal-width conversions
  also return independent owners. Two-state conversion preserves width/sign
  and known bits, clears X/Z positions to zero, and returns a new owner.
* Arithmetic and bitwise binary operations use maximum operand width and are
  signed only if both operands are signed. Arithmetic containing X/Z is all X.
  Known zero dominates AND; known one dominates OR. Logical truth can be true
  despite unknown other bits; logical equality can be false on a known mismatch.
  Case equality compares X and Z literally. Ambiguous integral mux keeps equal
  known bits; Z/Z merges to X. A known-arm mux preserves Z.
* `sv4_same` compares zero-padded legacy-plane words and deliberately ignores
  width and sign. For canonical payloads this compares stored states.
  Shape-sensitive users must compare neutral width/sign separately.
  Use logical/case comparison for HDL semantics, never descriptor byte equality.

Division truncates the quotient toward zero, remainder has the dividend sign,
zero divisor yields X, and minimum signed value divided by -1 wraps modulo
width. Shifts must check unknown, huge and at/above-width counts before calling
GMP kernels. These requirements belong to V05, not a claim of prototype coverage.
Strengths, net modes, driver identity, scheduling and two-state declaration
policy belong to containing objects, not the packed numeric descriptor. A
currently known four-state variable must still accept a later X/Z write.

## Four-state encodings and boundaries

The table describes canonical payloads. All word formats below are
least-significant-word first. Public bridge words
are always 64 bits, independent of host endian, `long` size or GMP limb width.
Foreign 32-bit vector words must split/join each 64-bit word with shifts, never
reinterpret `mp_limb_t*` or assume a foreign vector has the same layout.

| State | sv4 API code | legacy `(bits,x,z)` | GMP `(A,B)` | Rust `(unknown,value)` | DPI scalar | VPI/DPI vector `(aval,bval)` |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 0 | (0,0,0) | (0,0) | (0,0) | 0 | (0,0) |
| 1 | 1 | (1,0,0) | (1,0) | (0,1) | 1 | (1,0) |
| X | 2 | (0,1,0) | (1,1) | (1,0) | 3 | (1,1) |
| Z | 3 | (0,0,1) | (0,1) | (1,1) | 2 | (0,1) |

Legacy live planes require disjoint X/Z and zero top padding. Constructors
`sv4_from_limbs`/`sv4_from_masks` historically copy otherwise irrelevant bits
under unknown masks; callers should supply canonical masks. New bridge imports
canonicalize them and give X priority if X/Z overlap. Do not change an existing
constructor's invalid-input behavior as part of an adapter migration.

GMP stores `bits=A&~B`, `x=A&B`, `z=~A&B`; import is
`A=(bits&~(x|z))|x`, `B=x|z`. Widths 1..64 use inline words; wider storage uses
exact-width 32- or 64-bit `mp_limb_t` arrays with zero nails. B can be absent for
a currently known payload; that does not alter its HDL declaration type.

Rust `core::value::ValueData::Vector` stores `value_words` V and
`unknown_words` U: legacy `bits=V&~U`, `x=U&~V`, `z=U&V`; GMP `A=V^U`, `B=U`.
Conversely `U=x|z`, `V=bits|z` (canonical legacy), or `U=B`, `V=A^B` (GMP).
Validate width, exact word count and top padding at serialization/import
boundaries. Source spelling/radix strings and `ScalarValue::DontCare` retain
their frontend meaning; wildcard masks are not a fifth stored sv4 state.

VPI and DPI vectors use `aval=bits|x`, `bval=x|z`;
import uses `bits=aval&~bval`, `x=aval&bval`, `z=~aval&bval`.
Vector encoding matches internal GMP A/B logically, but not its memory layout.
DPI scalar codes reverse sv4's X/Z codes: use `llg_sv4_state_to_dpi` and
`llg_sv4_state_from_dpi`, with input codes restricted to 0..3.

## Borrowing, mutation, ownership and identity

Every packed by-value argument and `const sv4_t*` input borrows only for the
synchronous call, unless the operation explicitly consumes it. Every returned
`sv4_t` owns independent storage, including identity, clone, same-width cast,
resize, known-arm mux, reference reads and container reads. This rule also
holds for inline values: mutation of a result must never mutate an input.

| Operation | Contract and supported aliasing |
| --- | --- |
| Constructors, `SV4_C/S/X/Z/INIT` | Fresh owner. Raw input arrays borrow for the call. These macros are runtime calls, not static literals. `SV4_EMPTY` is the static initializer. |
| `sv4_clone` | Deep independent copy; input remains live. |
| `sv4_copy`, `sv4_assign` | Replace an initialized destination with a deep copy. Exact self-copy/self-assignment is supported. Allocate before releasing old storage. |
| `sv4_move` | Release destination, transfer ownership, reset source to empty. Exact self-move is a no-op. |
| `sv4_replace` | Consume a fresh returned owner. Named owners use move so the old descriptor is reset. Do not rely on legacy's incidental self-borrow/pointer-equality optimization. |
| `sv4_destroy`, `sv4_destroy_array` | Release owners and reset to empty; repeated destruction of an empty owner is safe. |
| Existing selected writes | Borrow RHS; snapshot exact/overlapping selected aliases before modifying the target. |
| Neutral setters/imports | Mutate only an initialized owner, keep width/address/sign, publish no scheduler notifications. Inputs are scalar copies or external buffers, never private payload aliases. |
| Explicit GMP `*_into` workspace operations | Exact destination/operand aliases are supported when documented. Scratch is caller-owned, thread-confined, separately accounted, and cannot overlap value payloads. |

Plain struct assignment is permitted only as a synchronous transient borrow or
an explicit transfer with the previous owner reset. A borrow must not be
destroyed, retained across source mutation, or used after move/replace/destroy.
Neither inline copies nor wide pointers make retained borrows safe.
Partially overlapping fabricated owners are unsupported. All deep reads return
owners; no implicit reference counting or arena lifetime excuses a leaked copy.

Cell identity is the address of the containing initialized descriptor or the
existing stable reference/container cell identity, **never** the payload pointer.
Replacement, promotion and compaction can relocate payloads. Queued NBA, force,
clocking, history, monitor, VPI and waveform users must capture independent values
and retain their destination cell through existing scope/pin rules. Do not move
the cell itself while a descriptor/reference retains its address. Scope unwind
and publication obligations remain in the ownership guide.

## Neutral bridge for feature agents

Legacy implementations are `static inline`; shape queries and in-range plane
loads reduce to the original field operations. No backend dispatch is involved.
GMP declarations/implementations are exercised through the prototype; production
V02/V03 provide the equivalent bridge with the reserved production symbol prefix.

| API | Use and limits |
| --- | --- |
| `llg_sv4_width`, `llg_sv4_signed`, `llg_sv4_words` | Shape/sign and ceil(width/64) word count. Do not assume descriptor size. |
| `llg_sv4_set_signed` | Metadata-only boolean sign update; no extension/coercion. Use cast/resize for value conversion. |
| `llg_sv4_state`, `llg_sv4_set_state` | 0/1/2=X/3=Z; out-of-range read is X, write is a no-op. Setter state must be 0..3. |
| `llg_sv4_word`, `llg_sv4_set_word` | Read one `LLG_SV4_BITS/X/Z` plane; plane must be one of these constants. Write all three scalar masks together. Out-of-range reads zero/write no-op; top padding masked, unknown positions canonicalized. |
| `llg_sv4_vpi_word`, `llg_sv4_set_vpi_word` | Read/write copied 64-bit `llg_sv4_vpi_word_t {aval,bval}`. |
| `llg_sv4_export_words`, `llg_sv4_import_words` | Copy ranges of `llg_sv4_word_t {bits,x,z}`, starting at word index `first`. |
| `llg_sv4_export_vpi_words`, `llg_sv4_import_vpi_words` | Equivalent ranges in VPI/DPI vector encoding. |
| `llg_sv4_has_x`, `llg_sv4_has_z`, existing `sv4_is_unknown` | Presence tests for X, Z, or either. |
| `llg_sv4_state_to_dpi`, `llg_sv4_state_from_dpi` | Scalar code conversion only; validate foreign codes first. |

Range exports write all `count` entries, padding beyond width with zero. Imports
ignore entries beyond width and preserve untouched words. Overflowing `first`
does not wrap into valid storage. A zero count permits NULL buffers; nonzero
counts require appropriately sized external arrays. `llg_sv4_word_range` is an
implementation helper for clipping, not a retained view. Callers supply bounded
counts. The bridge does not notify, resolve strengths, enforce destination
two-state policy, or resize a cell.

### How feature agents use values

Initialize cells with `SV4_EMPTY`, construct or clone inputs, query shape/state
through the bridge, and mutate through setters or existing selection operations.
Use external word copies for scanners, formatters, waveforms and foreign vector
adapters. Convert assignment with source-sign cast, then apply two-state coercion
when the **destination declaration** requires it. Install the result with move
or replace in temporary/contained owners. For a signal write, pass the prepared
owner to the existing scheduler write API so it captures the old value, installs
the new value and publishes together. Do not modify the signal before that call.
Its packed input borrows; release it after the call, or register its owner in a
scope if publication can cancel or exit through a callback.

Do not access `sv4_t`'s `.bits/.x/.z/.width/.is_signed` outside backend code, take writable
plane pointers, fabricate borrowed descriptors over arrays, use encoded-byte
equality, or embed numeric `sizeof(sv4_t)`/frame-offset constants. Ordinary C
`sizeof`/`_Alignof` of the selected type is allowed in native layout code; emitted
frame metadata must be selected and asserted by V07. V06 migrates representation
consumers; V05 implements operation families; V08 audits retained owner graphs.
Keep new feature work on legacy through these APIs while those tasks proceed.
