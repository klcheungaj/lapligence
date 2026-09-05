// llg_value.h — four-state value model and numeric conversions for the llg
// Verilog simulator's generated C11 models.
//
// Values
// ------
// `sv4_t` models a 4-state vector of at most LLG_MAX_WIDTH (1024) bits, stored
// as three parallel arrays of 64-bit limbs: bit i lives in `bits[i/64]` /
// `x[i/64]` / `z[i/64]` at position `i%64`.  The invariant x & z == 0 holds:
// bit i is X iff `(x[i/64] >> (i%64)) & 1`, Z iff `(z[i/64] >> (i%64)) & 1`,
// otherwise its value is `(bits[i/64] >> (i%64)) & 1`.  For expression
// semantics Z behaves like X in every op that propagates unknown bits (LRM
// 11.4.5); X and Z are only distinguished by `$display`, casez/casex wildcard
// matching, `===`/`!==` and the identity/copy ops (mux with a known select,
// selects, resize, concat) which carry Z through.  Every operation keeps limbs
// above the vector's width zero, and masks the top partial limb to the width.
//
#ifndef LLG_VALUE_H
#define LLG_VALUE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ── 4-state values ────────────────────────────────────────────────────────────

#define LLG_MAX_WIDTH 1024u
#define LLG_LIMBS ((LLG_MAX_WIDTH + 63u) / 64u) /* 16 */

typedef struct {
    uint64_t bits[LLG_LIMBS]; // known bits; valid where x/z bits are 0
    uint64_t x[LLG_LIMBS];    // X bits (unknown)
    uint64_t z[LLG_LIMBS];    // Z bits (high-impedance); x & z == 0
    uint16_t width;            // vector width (0..LLG_MAX_WIDTH)
    int8_t is_signed;          // signedness for resize/compare
} sv4_t;

// Compile-time bit mask for a width literal (<= 64).
#define LLG_MASK(w) ((w) >= 64 ? ~0ULL : ((1ULL << (w)) - 1))

// Build a value from single-limb bit/x/z masks.  Limbs above limb 0 are zero.
// `SV4_C`/`SV4_S`/`SV4_X` clamp the width to 64 — they fill limb 0 only and are
// meant for the code generator's <= 64-bit constants (wider values use
// `sv4_from_limbs` / the wide codegen follow-up).
#define SV4_INIT(b, x, z, w, s) \
    ((sv4_t){ { [0] = (uint64_t)(b) }, { [0] = (uint64_t)(x) }, \
              { [0] = (uint64_t)(z) }, \
              (uint16_t)(w), (int8_t)(s) })
// Unsigned / signed clean value with `w` bits.
#define SV4_C(b, w) SV4_INIT((b), 0, 0, ((w) > 64u ? 64u : (w)), 0)
#define SV4_S(b, w) SV4_INIT((b), 0, 0, ((w) > 64u ? 64u : (w)), 1)
// All-X value of `w` bits (constant expression for literal `w`).
#define SV4_X(w) SV4_INIT(0, LLG_MASK(w), 0, ((w) > 64u ? 64u : (w)), 0)
// All-Z value of `w` bits (constant expression for literal `w`).
#define SV4_Z(w) SV4_INIT(0, 0, LLG_MASK(w), ((w) > 64u ? 64u : (w)), 0)

// ── Value constructors / inspectors ───────────────────────────────────────────

sv4_t sv4_x(uint16_t width, int8_t is_signed);
sv4_t sv4_from_u64(uint64_t v, uint16_t width, int8_t is_signed);
sv4_t sv4_from_i64(int64_t v, uint16_t width);
double sv4_to_real(sv4_t v);
sv4_t sv4_from_real(double v, uint16_t width, int8_t is_signed);
sv4_t sv4_rtoi(double v);
sv4_t sv4_realtobits(double v);
// Dynamic X/Z input bits have no real representation and contribute zero.
double sv4_bitstoreal(sv4_t v);
sv4_t sv4_shortrealtobits(double v);
double sv4_bitstoshortreal(sv4_t v);
int llg_real_to_bool(double v);
// Build from raw limb arrays (any may be NULL to zero-fill); the top partial
// limb is masked to `width` and the width is clamped to LLG_MAX_WIDTH.
sv4_t sv4_from_limbs(const uint64_t* bits, const uint64_t* x, const uint64_t* z,
                     uint16_t width, int8_t is_signed);
sv4_t sv4_resize(sv4_t v, uint16_t width, int8_t is_signed);
// Value-preserving conversion (LRM 1800-2009 §6.24.1 / §10.7): widening
// extends by the SOURCE's signedness (`v.is_signed`), narrowing truncates;
// the result carries `is_signed`.  Unlike `sv4_resize`, whose extension
// follows the passed flag, an unsigned source zero-extends even into a
// signed target and a signed source sign-extends even into an unsigned one.
sv4_t sv4_cast(sv4_t v, uint16_t width, int8_t is_signed);
// All `width` bits set to one literal bit value: bit 0, bit 1, bit 2 = X,
// or bit 3 = Z.
sv4_t sv4_fill(uint8_t bit, uint16_t width, int8_t is_signed);
sv4_t sv4_clog2(sv4_t v);
sv4_t sv4_countones(sv4_t v);     // signed 32-bit count of known one bits
sv4_t sv4_onehot(sv4_t v, int allow_zero); // one-bit predicate, X/Z ignored

int sv4_is_unknown(sv4_t v);      // any bit X or Z
int sv4_to_bool(sv4_t v);         // != 0 with no unknown bits, else 0
uint64_t sv4_to_u64(sv4_t v);     // low limb; meaningful only when width <= 64
int64_t sv4_to_i64(sv4_t v);      // two's-complement interpretation of low bits
int sv4_fits_i64(sv4_t v);        // exact signed conversion is representable
int sv4_same(sv4_t a, sv4_t b);   // bits + x + z equal (ignores width/signed)

// Format one value into `buf` (NUL-terminated).  `fmt` is 'd', 'h', 'b' or 'o'.
// %b prints all width bits: 'x' for X bits and 'z' for Z bits; %h prints
// ceil(width/4) digits ('x' if any bit of the nibble is X, else 'z' if any is
// Z); %o likewise in octal; %d prints 'x' when any bit is X or Z.
void sv4_format(char fmt, sv4_t v, char* buf, size_t cap);
// Unsigned decimal via long division across limbs; any unknown bit -> "x".
// A signed value (`is_signed`) with the sign bit set prints '-' followed by
// its two's-complement magnitude (`~v + 1` within the value's width).
void sv4_to_dec_string(sv4_t v, char* buf, size_t cap);

// ── Arithmetic / logic ops (IEEE 1364 semantics) ──────────────────────────────
//
// Result widths are self-determined exactly like src/core/elab.rs: arithmetic
// and bitwise ops use max operand width, shifts keep the LHS width, compares /
// reductions / logical ops yield 1 bit.  Unknown operand bits propagate: any
// unknown bit makes arithmetic results all-X; 0 dominates AND and 1 dominates
// OR per bit; a shift with unknown amount yields all-X.

sv4_t sv4_add(sv4_t a, sv4_t b);
sv4_t sv4_sub(sv4_t a, sv4_t b);
sv4_t sv4_mul(sv4_t a, sv4_t b);
sv4_t sv4_div(sv4_t a, sv4_t b);
sv4_t sv4_mod(sv4_t a, sv4_t b);
sv4_t sv4_pow(sv4_t a, sv4_t b);
sv4_t sv4_neg(sv4_t a);              // unary minus
sv4_t sv4_bitneg(sv4_t a);           // ~
sv4_t sv4_lognot(sv4_t a);           // !
sv4_t sv4_and(sv4_t a, sv4_t b);     // &
sv4_t sv4_or(sv4_t a, sv4_t b);      // |
sv4_t sv4_xor(sv4_t a, sv4_t b);     // ^
sv4_t sv4_xnor(sv4_t a, sv4_t b);    // ~^
sv4_t sv4_logand(sv4_t a, sv4_t b);  // &&
sv4_t sv4_logor(sv4_t a, sv4_t b);   // ||
sv4_t sv4_reduce_and(sv4_t a);       // &a
sv4_t sv4_reduce_nand(sv4_t a);
sv4_t sv4_reduce_or(sv4_t a);        // |a
sv4_t sv4_reduce_nor(sv4_t a);
sv4_t sv4_reduce_xor(sv4_t a);       // ^a
sv4_t sv4_reduce_xnor(sv4_t a);
sv4_t sv4_shl(sv4_t a, sv4_t b);     // <<
sv4_t sv4_shr(sv4_t a, sv4_t b);     // >>
sv4_t sv4_ashl(sv4_t a, sv4_t b);    // <<<
sv4_t sv4_ashr(sv4_t a, sv4_t b);    // >>>
sv4_t sv4_eq(sv4_t a, sv4_t b);      // == (X when any operand bit X/Z)
sv4_t sv4_neq(sv4_t a, sv4_t b);
sv4_t sv4_case_eq(sv4_t a, sv4_t b); // === (never X; X/Z compared literally)
sv4_t sv4_case_neq(sv4_t a, sv4_t b);
// ==?/!=?: X/Z bits in rhs are wildcards; lhs X/Z on cared bits propagate X.
sv4_t sv4_wild_eq(sv4_t lhs, sv4_t rhs);
sv4_t sv4_wild_neq(sv4_t lhs, sv4_t rhs);
// casez/casex wildcard match (never X; 1-bit result), per LRM 12.5.1.  Both
// resize the operands to max width (zero-extend) and test bits LSB-up:
//   casez: item z/? -> don't-care; item x -> matches selector x only;
//          item known -> matches only an equal selector bit (sel x/z -> no).
//   casex: item x/z/? -> don't-care; item known -> matches unless the
//          selector holds the opposite known bit (sel x/z -> match).
sv4_t sv4_casez_eq(sv4_t sel, sv4_t item);
sv4_t sv4_casex_eq(sv4_t sel, sv4_t item);
sv4_t sv4_lt(sv4_t a, sv4_t b);
sv4_t sv4_le(sv4_t a, sv4_t b);
sv4_t sv4_gt(sv4_t a, sv4_t b);
sv4_t sv4_ge(sv4_t a, sv4_t b);
sv4_t sv4_mux(sv4_t sel, sv4_t a, sv4_t b);
sv4_t sv4_concat(sv4_t hi, sv4_t lo);     // hi is the MS part
sv4_t sv4_repeat(sv4_t pat, uint64_t n);  // {n{pat}}
sv4_t sv4_part_select(sv4_t v, int64_t left, int64_t right); // handles reversed ranges
void sv4_part_select_set(sv4_t* tgt, int64_t left, int64_t right, sv4_t value);
sv4_t sv4_bit_select(sv4_t v, uint64_t i);
void sv4_bit_select_set(sv4_t* tgt, uint64_t i, sv4_t value);
sv4_t sv4_idx_part_select(sv4_t v, uint64_t base, uint16_t width, int neg);
void sv4_idx_part_select_set(sv4_t* tgt, uint64_t base, uint16_t width, int neg,
                             sv4_t value);

#ifdef __cplusplus
}
#endif

#endif // LLG_VALUE_H
