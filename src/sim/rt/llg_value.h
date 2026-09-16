// llg_value.h — four-state value model and numeric conversions for the llg
// Verilog simulator's generated C11 models.
//
// Values
// ------
// `sv4_t` owns three exact-width planes in one allocation. Bit i lives in
// bits[i/64], x[i/64], z[i/64] at position i%64. X and Z remain distinct
// (x & z == 0). No model-wide capacity is embedded in a value. For expression
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

#define LLG_VALUE_ABI_VERSION 3u
#define LLG_SUPPORTED_WIDTH_LIMIT (1u << 20)

// A live value owns exactly one allocation, addressed by bits; x and z are
// interior pointers. Width zero owns nothing. Width must be strictly less than
// LLG_SUPPORTED_WIDTH_LIMIT. Every constructor and value-returning operation
// returns an independent owner. By-value arguments are BORROWED, not consumed.
//
// Initialize destinations with SV4_EMPTY (or a constructor). Do not copy owners
// with assignment/memcpy: use clone/copy/move. A plain descriptor copy is only a
// temporary borrow and must never be destroyed or retained across replacement.
// Destroy values at the end of their containing object's lifetime; destruction
// resets to empty and is idempotent. No compiler cleanup extensions are used.
typedef struct {
    uint64_t* bits;
    uint64_t* x;
    uint64_t* z;
    uint32_t width;
    int8_t is_signed;
} sv4_t;

#define SV4_EMPTY {NULL, NULL, NULL, 0, 0}

sv4_t sv4_zero(uint32_t width, int8_t is_signed);
sv4_t sv4_clone(const sv4_t* source);
// Initialized destination; copy is deep and supports self-copy. Move releases
// the previous destination, transfers ownership, and empties the source.
void sv4_copy(sv4_t* destination, const sv4_t* source);
void sv4_move(sv4_t* destination, sv4_t* source);
// Consume a freshly returned owner. Use move for a named source so it is reset.
void sv4_replace(sv4_t* destination, sv4_t owned);
// Borrowed-value shorthand for copy; useful when the source is an expression.
void sv4_assign(sv4_t* destination, sv4_t source);
void sv4_destroy(sv4_t* value);
void sv4_destroy_array(sv4_t* values, size_t count);
size_t sv4_bytes(const sv4_t* value);
// Masked one-limb constructor, including allocation-free width zero.
sv4_t sv4_from_masks(uint64_t bits, uint64_t x, uint64_t z,
                     uint32_t width, int8_t is_signed);

typedef struct llg_queue_t llg_queue_t;
typedef sv4_t (*llg_queue_ref_read_fn)(const llg_queue_t* queue,
                                       uint64_t identity);
typedef int (*llg_queue_ref_write_fn)(llg_queue_t* queue, uint64_t identity,
                                      sv4_t value);

// Canonical lvalue descriptor used by subroutine `ref` arguments.  The
// descriptor always names the original packed storage (`base`); selected
// aliases retain their source bounds so reads and writes remain immediate and
// do not require copy-in/copy-out temporaries.
typedef enum {
    LLG_REF_WHOLE = 0,
    LLG_REF_BIT = 1,
    LLG_REF_PART = 2,
    LLG_REF_INDEXED = 3,
    LLG_REF_ARRAY = 4,
    LLG_REF_QUEUE = 5,
} llg_ref_kind_t;

typedef struct {
    sv4_t* base;
    llg_queue_t* queue;
    uint32_t width;
    int8_t is_signed;
    uint8_t two_state;
    uint8_t kind;
    int64_t left;
    int64_t right;
    uint64_t index;
    uint32_t indexed_width;
    uint8_t indexed_negative;
    uint64_t array_size;
    uint64_t queue_identity;
    llg_queue_ref_read_fn queue_read;
    llg_queue_ref_write_fn queue_write;
    void* retained;
    sv4_t (*retained_read)(const void*);
    int (*retained_write)(void*, sv4_t);
} llg_ref_t;

sv4_t llg_ref_read(const llg_ref_t* ref);

// Net resolution modes.  The pure resolver has no scheduler
// dependency; llg_rt.c is responsible for publishing changes to waiters.
enum {
    LLG_RESOLVE_WIRE = 0,
    LLG_RESOLVE_WAND = 1,
    LLG_RESOLVE_WOR = 2,
    LLG_RESOLVE_TRI0 = 3,
    LLG_RESOLVE_TRI1 = 4,
    LLG_RESOLVE_SUPPLY0 = 5,
    LLG_RESOLVE_SUPPLY1 = 6,
};

// IEEE 1800-2009 Table 28-7 strength levels. Continuous assignments use
// HIGHZ, WEAK, PULL, STRONG, or SUPPLY; the intermediate levels are retained
// so the resolver's representation matches the standard's ordered scale.
enum {
    LLG_STRENGTH_HIGHZ = 0,
    LLG_STRENGTH_SMALL = 1,
    LLG_STRENGTH_MEDIUM = 2,
    LLG_STRENGTH_WEAK = 3,
    LLG_STRENGTH_LARGE = 4,
    LLG_STRENGTH_PULL = 5,
    LLG_STRENGTH_STRONG = 6,
    LLG_STRENGTH_SUPPLY = 7,
};

// Compile-time bit mask for a width literal (<= 64).
#define LLG_MASK(w) ((w) >= 64 ? ~0ULL : ((1ULL << (w)) - 1))

// These are runtime constructors, NOT static initializers. Each invocation
// creates an owner. Static model cells start SV4_EMPTY and are initialized by
// generated startup code. Use from_limbs/fill for multi-limb literals.
#define SV4_INIT(b, x, z, w, s) \
    sv4_from_masks((uint64_t)(b), (uint64_t)(x), (uint64_t)(z), (w), (s))
#define SV4_C(b, w) sv4_from_u64((uint64_t)(b), (w), 0)
#define SV4_S(b, w) sv4_from_u64((uint64_t)(b), (w), 1)
#define SV4_X(w) sv4_x((w), 0)
#define SV4_Z(w) sv4_fill(3, (w), 0)

// ── Value constructors / inspectors ───────────────────────────────────────────

sv4_t sv4_x(uint32_t width, int8_t is_signed);
sv4_t sv4_from_u64(uint64_t v, uint32_t width, int8_t is_signed);
sv4_t sv4_from_i64(int64_t v, uint32_t width);
double sv4_to_real(sv4_t v);
sv4_t sv4_from_real(double v, uint32_t width, int8_t is_signed);
sv4_t sv4_rtoi(double v);
sv4_t sv4_realtobits(double v);
// Dynamic X/Z input bits have no real representation and contribute zero.
double sv4_bitstoreal(sv4_t v);
sv4_t sv4_shortrealtobits(double v);
double sv4_bitstoshortreal(sv4_t v);
int llg_real_to_bool(double v);
// Build from raw limb arrays (any may be NULL to zero-fill); the top partial
// limb is masked to `width`. Widths reaching the supported limit fail rather
// than silently truncating. Inputs are borrowed only for the duration of the call.
sv4_t sv4_from_limbs(const uint64_t* bits, const uint64_t* x, const uint64_t* z,
                     uint32_t width, int8_t is_signed);
sv4_t sv4_resize(sv4_t v, uint32_t width, int8_t is_signed);
// Value-preserving conversion (LRM 1800-2009 §6.24.1 / §10.7): widening
// extends by the SOURCE's signedness (`v.is_signed`), narrowing truncates;
// the result carries `is_signed`.  Unlike `sv4_resize`, whose extension
// follows the passed flag, an unsigned source zero-extends even into a
// signed target and a signed source sign-extends even into an unsigned one.
sv4_t sv4_cast(sv4_t v, uint32_t width, int8_t is_signed);
// Packed four-state to two-state conversion: X and Z bits become zero while
// known bits, width, and signedness are preserved.
sv4_t sv4_to_two_state(sv4_t v);
// All `width` bits set to one literal bit value: bit 0, bit 1, bit 2 = X,
// or bit 3 = Z.
sv4_t sv4_fill(uint8_t bit, uint32_t width, int8_t is_signed);
sv4_t sv4_clog2(sv4_t v);
sv4_t sv4_countones(sv4_t v);     // signed 32-bit count of known one bits
sv4_t sv4_onehot(sv4_t v, int allow_zero); // one-bit predicate, X/Z ignored

int sv4_is_unknown(sv4_t v);      // any bit X or Z
int sv4_to_bool(sv4_t v);         // != 0 with no unknown bits, else 0
// Normalize a repeat count without truncating wide values. Unknown, Z, and
// negative signed counts mean zero iterations; positive counts are unsigned.
sv4_t sv4_repeat_count(sv4_t v);
uint64_t sv4_to_u64(sv4_t v);     // low limb; meaningful only when width <= 64
// Convert a packed index without silently discarding upper bits.  Unknown,
// negative, and wider-than-uint64 values return UINT64_MAX (always out of
// range for an admitted packed value).
uint64_t sv4_to_index(sv4_t v);
// Procedural delays: X/Z is zero; negative packed values convert to unsigned
// 64-bit time. Real values round to local precision before scheduler scaling.
uint64_t sv4_delay_ticks(sv4_t value, uint64_t unit_ticks);
uint64_t sv4_real_delay_ticks(double value, uint64_t unit_ticks,
                              uint64_t precision_ticks);
// Exact signed host index conversion honoring the packed value's signedness.
// Returns zero for X/Z or an out-of-range value without modifying `result`.
int sv4_to_index_i64(sv4_t v, int64_t* result);
// Convert a runtime-computed packed width.  Invalid, unknown, negative, or
// over-capacity values terminate explicitly rather than truncating.
uint32_t sv4_checked_width(sv4_t v);
int64_t sv4_to_i64(sv4_t v);      // two's-complement interpretation of low bits
int sv4_fits_i64(sv4_t v);        // exact signed conversion is representable
int sv4_same(sv4_t a, sv4_t b);   // bits + x + z equal (ignores width/signed)
// Resolve per-driver contributions at one net.  Z is absent/neutral; an
// all-Z bit stays Z.  WAND gives 0 dominance, WOR gives 1 dominance, and
// WIRE reports conflicting known values as X. TRI0/TRI1 apply their pull
// only to all-Z bits. SUPPLY0/SUPPLY1 model their implicit supply source.
sv4_t sv4_resolve(const sv4_t* const* drivers, int n_drivers,
                  uint32_t width, int8_t is_signed, int mode);
// Resolve direct driver contributions with one strength endpoint for each
// logic value. An X contribution spans both endpoint ranges; therefore a
// known value is stable only when a known driver strictly dominates every
// possible opposite endpoint. WAND/WOR use the same ordered endpoints and
// apply their wired tie rule. TRI0/TRI1 and SUPPLY0/SUPPLY1 add their
// implicit pull/supply source at the corresponding strength. Strength arrays
// contain n_drivers entries.
sv4_t sv4_resolve_strengths(const sv4_t* const* drivers,
                            const uint8_t* strength0,
                            const uint8_t* strength1, int n_drivers,
                            uint32_t width, int8_t is_signed, int mode);

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
sv4_t sv4_logimpl(sv4_t a, sv4_t b); // ->
sv4_t sv4_logequiv(sv4_t a, sv4_t b); // <->
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
// Logical equality: a known mismatch yields 0 even if other bits are X/Z;
// otherwise any X/Z yields X.
sv4_t sv4_eq(sv4_t a, sv4_t b);
sv4_t sv4_neq(sv4_t a, sv4_t b);
sv4_t sv4_case_eq(sv4_t a, sv4_t b); // === (never X; X/Z compared literally)
sv4_t sv4_case_neq(sv4_t a, sv4_t b);
// Navigate a declaration-ordered enum table. Invalid/unknown receivers
// return default_value; duplicate values select the last matching declaration.
sv4_t sv4_enum_navigate(sv4_t current, sv4_t step, const sv4_t* values,
                        uint32_t count, sv4_t default_value, int direction);
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
// Inclusive inside-range match. Unknown relational results propagate unless
// one comparison is definitively false.
sv4_t sv4_inside_range(sv4_t value, sv4_t low, sv4_t high);
sv4_t sv4_mux(sv4_t sel, sv4_t a, sv4_t b);
sv4_t sv4_concat(sv4_t hi, sv4_t lo);     // hi is the MS part
sv4_t sv4_repeat(sv4_t pat, uint64_t n);  // {n{pat}}
// Packed streaming: right_to_left reverses slice-sized blocks; left-to-right
// preserves stream order. The result is unsigned and has value.width bits.
sv4_t sv4_stream(sv4_t value, uint32_t slice, int right_to_left);
// Inverse mapping used when a packed stream is an assignment target.
sv4_t sv4_unstream(sv4_t value, uint32_t slice, int right_to_left);
sv4_t sv4_part_select(sv4_t v, int64_t left, int64_t right); // handles reversed ranges
void sv4_part_select_set(sv4_t* tgt, int64_t left, int64_t right, sv4_t value);
sv4_t sv4_bit_select(sv4_t v, uint64_t i);
void sv4_bit_select_set(sv4_t* tgt, uint64_t i, sv4_t value);
sv4_t sv4_idx_part_select(sv4_t v, uint64_t base, uint32_t width, int neg);
void sv4_idx_part_select_set(sv4_t* tgt, uint64_t base, uint32_t width, int neg,
                             sv4_t value);
// Value-based variants preserve a signed negative base long enough to model
// partial out-of-range overlap (out-of-range read bits are X; writes are no-op).
sv4_t sv4_idx_part_select_value(sv4_t v, sv4_t base, uint32_t width, int neg);
void sv4_idx_part_select_set_value(sv4_t* tgt, sv4_t base, uint32_t width,
                                   int neg, sv4_t value);

#ifdef __cplusplus
}
#endif

#endif // LLG_VALUE_H
