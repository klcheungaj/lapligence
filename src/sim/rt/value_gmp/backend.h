#ifndef LLG_SV4_COMPACT_BACKEND_H
#define LLG_SV4_COMPACT_BACKEND_H

#include <stddef.h>
#include <stdint.h>
#include <math.h>
#include <string.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LLG_GMP_VALUE_ABI_VERSION 5u
#define LLG_GMP_SUPPORTED_WIDTH_LIMIT (UINT32_C(1) << 20)
#ifndef LLG_SV4_MUL_FULL_THRESHOLD
#define LLG_SV4_MUL_FULL_THRESHOLD 128u
#endif

/* Unique owner. By-value operands borrow; results own. B exists exactly when
 * there are X/Z bits. Only wide.a is freed; wide.b is an interior pointer. */
typedef struct {
    union {
        struct {
            uint64_t a, b;
        } small;
        struct {
            uint64_t *a, *b;
        } wide;
    } data;
    uint32_t width;
    int8_t is_signed;
} llg_gmp_sv4_t;
#define LLG_GMP_SV4_EMPTY {{{0, 0}}, 0, 0}
/* Constant-width <=64 initializer, also usable in static storage. */
#define LLG_GMP_SV4_LITERAL(bits, x, z, w, s)                                                      \
    {{{(((uint64_t)(bits) & ~((uint64_t)(x) | (uint64_t)(z))) | (uint64_t)(x)) & LLG_GMP_MASK(w),  \
       ((uint64_t)(x) | (uint64_t)(z)) & LLG_GMP_MASK(w)}},                                        \
     (w),                                                                                          \
     ((s) != 0)}
#define LLG_GMP_MASK(w)                                                                            \
    ((w) == 0 ? UINT64_C(0) : (w) >= 64 ? UINT64_MAX : UINT64_MAX >> (64u - (w)))

typedef struct {
    uint64_t bits, x, z;
} llg_gmp_sv4_word_t;
typedef struct {
    uint64_t aval, bval;
} llg_gmp_sv4_vpi_word_t;

typedef llg_gmp_sv4_t g4_t;

/* V05d: V06 consumer primitives. Borrowed inputs, fixed shape/sign; no
 * scratch owners. Only actual X/Z writes can promote B; mutations compact it.
 * Range copy/same require a fitting interval and independent owners or self at
 * offset zero. Arithmetic requires known inputs; digit count is in 1..4.
 * Foreign records have stride >=8 and hold count records without size overflow. */
void llg_gmp_sv4_masked_merge(g4_t* target, g4_t source, g4_t mask);
void llg_gmp_sv4_masked_copy(g4_t* target, g4_t source, g4_t mask);
int llg_gmp_sv4_masked_same(g4_t a, g4_t b, const g4_t* mask);
int llg_gmp_sv4_range_same(g4_t target, uint32_t offset, g4_t source);
void llg_gmp_sv4_range_copy(g4_t* target, uint32_t offset, g4_t source);
void llg_gmp_sv4_range_fill(g4_t* target, uint32_t low, uint32_t count, unsigned state);
void llg_gmp_sv4_mask_remove(g4_t* target, g4_t removed);
void llg_gmp_sv4_mask_top_wide(g4_t* value);
void llg_gmp_sv4_mul_add_known_wide(g4_t* value, uint32_t factor, uint32_t addend);
void llg_gmp_sv4_negate_known_wide(g4_t* value);
void llg_gmp_sv4_two_state_inplace_wide(g4_t* value);
void llg_gmp_sv4_append_digit_wide(g4_t* value, unsigned count, unsigned state, unsigned digit);
void llg_gmp_sv4_export_vpi32(g4_t value, void* output, size_t count, size_t stride);
void llg_gmp_sv4_import_vpi32(g4_t* value, const void* input, size_t count, size_t stride);
int llg_gmp_sv4_same_vpi_words(g4_t value, const llg_gmp_sv4_vpi_word_t* words, size_t count);
void llg_gmp_sv4_export_text(g4_t value, uint32_t width, char* output);
/* End V05d consumer declarations. */

/* V05 S4/S5: selections, borrowed reference graphs and assembly. */
#include "reference_types.h"

llg_gmp_sv4_select_plan_t llg_gmp_sv4_select_plan_init(uint32_t storage_width);
// Single-stage plans used when a selected write must retain its coordinates.
// Invalid or wholly out-of-range selectors produce an empty valid interval.
llg_gmp_sv4_select_plan_t llg_gmp_sv4_select_plan_bit(uint32_t storage_width, uint64_t index);
llg_gmp_sv4_select_plan_t llg_gmp_sv4_select_plan_part(uint32_t storage_width, int64_t left,
                                                       int64_t right);
llg_gmp_sv4_select_plan_t llg_gmp_sv4_select_plan_indexed(uint32_t storage_width, g4_t base,
                                                          uint32_t width, int negative);
// All bases are borrowed. Unknown or unrepresentable bases select no bits.
void llg_gmp_sv4_select_plan_step(llg_gmp_sv4_select_plan_t* plan, g4_t base, uint32_t width);
// Read returns an independent unsigned owner, with X at missing positions.
g4_t llg_gmp_sv4_select_plan_read_wide(g4_t source, const llg_gmp_sv4_select_plan_t* plan);
// Return only the plan's valid contiguous interval. Reverse maps ascending
// declared part-selects into increasing storage-bit order.
g4_t llg_gmp_sv4_select_plan_slice_wide(g4_t source, const llg_gmp_sv4_select_plan_t* plan,
                                        int reverse);
// Set borrows source; supports aliasing and changes only the valid interval.
void llg_gmp_sv4_select_plan_set_wide(g4_t* destination, const llg_gmp_sv4_select_plan_t* plan,
                                      g4_t source);

// Pure validation; runtime-facing reference access reports a failed check.
int llg_gmp_ref_view_valid(const llg_gmp_ref_view_t* view, const g4_t* parent,
                           size_t* failed_check);
g4_t llg_gmp_ref_read(const llg_gmp_ref_t* ref);

g4_t llg_gmp_sv4_part_select_wide(g4_t v, int64_t left, int64_t right);
void llg_gmp_sv4_part_select_set_wide(g4_t* tgt, int64_t left, int64_t right, g4_t value);
g4_t llg_gmp_sv4_idx_part_select_wide(g4_t v, uint64_t base, uint32_t width, int neg);
void llg_gmp_sv4_idx_part_select_set_wide(g4_t* tgt, uint64_t base, uint32_t width, int neg,
                                          g4_t value);
g4_t llg_gmp_sv4_idx_part_select_value_wide(g4_t v, g4_t base, uint32_t width, int neg);
void llg_gmp_sv4_idx_part_select_set_value_wide(g4_t* tgt, g4_t base, uint32_t width, int neg,
                                                g4_t value);
g4_t llg_gmp_sv4_concat_wide(g4_t hi, g4_t lo);
g4_t llg_gmp_sv4_repeat_wide(g4_t pat, uint64_t n);
g4_t llg_gmp_sv4_repeat_count_wide(g4_t v);
g4_t llg_gmp_sv4_stream_wide(g4_t value, uint32_t slice, int right_to_left);
g4_t llg_gmp_sv4_unstream_wide(g4_t value, uint32_t slice, int right_to_left);
g4_t llg_gmp_sv4_array_conditional_merge_wide(g4_t a, g4_t b, g4_t element_default);
/* End V05 S4/S5 declarations. */
void llg_gmp_sv4_fail(const char* message);
g4_t llg_gmp_sv4_zero_wide(uint32_t width, int8_t sign);
g4_t llg_gmp_sv4_fill_wide(uint8_t state, uint32_t width, int8_t sign);
g4_t llg_gmp_sv4_clone_wide(const g4_t* source);
void llg_gmp_sv4_destroy_wide(g4_t* value);
void llg_gmp_sv4_copy_wide(g4_t* destination, const g4_t* source);
g4_t llg_gmp_sv4_from_limbs_wide(const uint64_t* bits, const uint64_t* x, const uint64_t* z,
                                 uint32_t width, int8_t sign);
g4_t llg_gmp_sv4_convert_wide(g4_t value, uint32_t width, int8_t sign, int extend);
g4_t llg_gmp_sv4_two_state_wide(g4_t value);
g4_t llg_gmp_sv4_binary_wide(g4_t a, g4_t b, unsigned op);
/* Private wide kernel: canonical known planes, equal width >64, op != 2. */
g4_t llg_gmp_sv4_binary_known_equal(const uint64_t* a, const uint64_t* b, uint32_t width,
                                    int8_t sign, unsigned op);
g4_t llg_gmp_sv4_unary_wide(g4_t value, int bitwise);
g4_t llg_gmp_sv4_compare_wide(g4_t a, g4_t b, unsigned op);
g4_t llg_gmp_sv4_mux_wide(g4_t a, g4_t b);
g4_t llg_gmp_sv4_divmod_wide(g4_t a, g4_t b, int remainder);
g4_t llg_gmp_sv4_pow_wide(g4_t a, g4_t b);
g4_t llg_gmp_sv4_clog2_wide(g4_t value);
int llg_gmp_sv4_truth_wide(g4_t value);
int llg_gmp_sv4_same_wide(g4_t a, g4_t b);
int llg_gmp_sv4_has_plane_wide(g4_t value, int z);
void llg_gmp_sv4_set_ab_wide(g4_t* value, size_t word, uint64_t a, uint64_t b);
void llg_gmp_sv4_import_words_wide(g4_t* value, size_t first, const llg_gmp_sv4_word_t* input,
                                   size_t count);
void llg_gmp_sv4_import_vpi_words_wide(g4_t* value, size_t first,
                                       const llg_gmp_sv4_vpi_word_t* input, size_t count);

static inline uint64_t g4_mask(uint32_t width) { return LLG_GMP_MASK(width); }
static inline uint32_t g4_maxw(g4_t a, g4_t b) { return a.width > b.width ? a.width : b.width; }
static inline void g4_width_check(uint32_t width) {
    if (width >= LLG_GMP_SUPPORTED_WIDTH_LIMIT)
        llg_gmp_sv4_fail("width reaches exclusive limit");
}
static inline uint32_t llg_gmp_sv4_width(g4_t v) { return v.width; }
static inline int8_t llg_gmp_sv4_signed(g4_t v) { return v.is_signed; }
static inline void llg_gmp_sv4_set_signed(g4_t* v, int8_t s) { v->is_signed = s != 0; }
static inline size_t llg_gmp_sv4_words(g4_t v) { return ((size_t)v.width + 63u) / 64u; }
static inline size_t llg_gmp_sv4_bytes(const g4_t* v) {
    return v->width <= 64 ? 0 : 8u * llg_gmp_sv4_words(*v) * (v->data.wide.b ? 2u : 1u);
}
static inline int llg_gmp_sv4_is_unknown(g4_t v) {
    return v.width <= 64 ? v.data.small.b != 0 : v.data.wide.b != NULL;
}
static inline g4_t g4_small(uint64_t a, uint64_t b, uint32_t w, int8_t s) {
    g4_t v = LLG_GMP_SV4_EMPTY;
    v.width = w;
    v.is_signed = s != 0;
    v.data.small.a = a & g4_mask(w);
    v.data.small.b = b & g4_mask(w);
    return v;
}
static inline g4_t llg_gmp_sv4_zero(uint32_t w, int8_t s) {
    return w <= 64 ? g4_small(0, 0, w, s) : llg_gmp_sv4_zero_wide(w, s);
}
static inline g4_t llg_gmp_sv4_fill(uint8_t state, uint32_t w, int8_t s) {
    if (state > 3)
        llg_gmp_sv4_fail("invalid scalar state");
    return w <= 64 ? g4_small(state == 1 || state == 2 ? UINT64_MAX : 0,
                              state >= 2 ? UINT64_MAX : 0, w, s)
                   : llg_gmp_sv4_fill_wide(state, w, s);
}
static inline g4_t llg_gmp_sv4_x(uint32_t w, int8_t s) { return llg_gmp_sv4_fill(2, w, s); }
static inline g4_t llg_gmp_sv4_from_u64(uint64_t bits, uint32_t w, int8_t s) {
    if (w <= 64)
        return g4_small(bits, 0, w, s);
    g4_t v = llg_gmp_sv4_zero_wide(w, s);
    v.data.wide.a[0] = bits;
    return v;
}
static inline g4_t llg_gmp_sv4_from_masks(uint64_t bits, uint64_t x, uint64_t z, uint32_t w,
                                          int8_t s) {
    if (w > 64)
        llg_gmp_sv4_fail("mask constructor requires width <=64");
    return g4_small((bits & ~(x | z)) | x, x | z, w, s);
}
static inline g4_t llg_gmp_sv4_from_limbs(const uint64_t* bits, const uint64_t* x,
                                          const uint64_t* z, uint32_t w, int8_t sign) {
    if (w > 64)
        return llg_gmp_sv4_from_limbs_wide(bits, x, z, w, sign);
    return llg_gmp_sv4_from_masks(w && bits ? bits[0] : 0, w && x ? x[0] : 0, w && z ? z[0] : 0, w,
                                  sign);
}
static inline g4_t llg_gmp_sv4_clone(const g4_t* v) {
    return v->width <= 64 ? *v : llg_gmp_sv4_clone_wide(v);
}
static inline void llg_gmp_sv4_destroy(g4_t* v) {
    if (!v)
        return;
    if (v->width > 64)
        llg_gmp_sv4_destroy_wide(v);
    *v = g4_small(0, 0, 0, 0);
}
static inline void llg_gmp_sv4_destroy_array(g4_t* v, size_t count) {
    for (size_t i = 0; i < count; ++i)
        llg_gmp_sv4_destroy(&v[i]);
}
static inline void llg_gmp_sv4_replace(g4_t* v, g4_t owned) {
    llg_gmp_sv4_destroy(v);
    *v = owned;
}
static inline void llg_gmp_sv4_move(g4_t* v, g4_t* source) {
    if (v == source)
        return;
    llg_gmp_sv4_replace(v, *source);
    *source = g4_small(0, 0, 0, 0);
}
static inline void llg_gmp_sv4_copy(g4_t* v, const g4_t* source) {
    if (v == source)
        return;
    if (v->width <= 64 && source->width <= 64) {
        *v = *source;
        return;
    }
    llg_gmp_sv4_copy_wide(v, source);
}
static inline void llg_gmp_sv4_assign(g4_t* v, g4_t source) { llg_gmp_sv4_copy(v, &source); }
static inline uint64_t g4_extend(uint64_t word, uint32_t source, uint32_t w, int sign) {
    if (sign && source && w > source && ((word >> (source - 1u)) & 1u))
        word |= ~g4_mask(source);
    return word & g4_mask(w);
}
static inline g4_t g4_convert(g4_t v, uint32_t w, int8_t s, int extend) {
    g4_width_check(w);
    if (v.width <= 64 && w <= 64)
        return g4_small(g4_extend(v.data.small.a, v.width, w, extend),
                        g4_extend(v.data.small.b, v.width, w, extend), w, s);
    return llg_gmp_sv4_convert_wide(v, w, s, extend);
}
static inline g4_t llg_gmp_sv4_resize(g4_t v, uint32_t w, int8_t s) {
    return g4_convert(v, w, s, s != 0);
}
static inline g4_t llg_gmp_sv4_cast(g4_t v, uint32_t w, int8_t s) {
    return g4_convert(v, w, s, v.is_signed);
}
static inline g4_t llg_gmp_sv4_from_i64(int64_t bits, uint32_t w) {
    return llg_gmp_sv4_cast(g4_small((uint64_t)bits, 0, 64, 1), w, 1);
}
static inline g4_t llg_gmp_sv4_to_two_state(g4_t v) {
    return v.width <= 64 ? g4_small(v.data.small.a & ~v.data.small.b, 0, v.width, v.is_signed)
                         : llg_gmp_sv4_two_state_wide(v);
}
static inline uint64_t llg_gmp_sv4_to_u64(g4_t v) {
    if (!v.width)
        return 0;
    uint64_t a = v.width <= 64 ? v.data.small.a : v.data.wide.a[0];
    uint64_t b = v.width <= 64 ? v.data.small.b : v.data.wide.b ? v.data.wide.b[0] : 0;
    return a & ~b;
}
static inline int g4_truth(g4_t v) {
    if (v.width > 64)
        return llg_gmp_sv4_truth_wide(v);
    return (v.data.small.a & ~v.data.small.b) ? 1 : v.data.small.b ? 2 : 0;
}
static inline int llg_gmp_sv4_to_bool(g4_t v) { return g4_truth(v) == 1; }
/* Operations 0..2 arithmetic, 3..6 bitwise. */
static inline g4_t g4_binary(g4_t a, g4_t b, unsigned op) {
    uint32_t w = g4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (w > 64) {
        if (op <= 2 && (llg_gmp_sv4_is_unknown(a) || llg_gmp_sv4_is_unknown(b)))
            return llg_gmp_sv4_x(w, s);
        if (op != 2 && a.width == b.width && !a.data.wide.b && !b.data.wide.b)
            return llg_gmp_sv4_binary_known_equal(a.data.wide.a, b.data.wide.a, w, s, op);
        return llg_gmp_sv4_binary_wide(a, b, op);
    }
    uint64_t aa = g4_extend(a.data.small.a, a.width, w, s),
             ab = g4_extend(a.data.small.b, a.width, w, s);
    uint64_t ba = g4_extend(b.data.small.a, b.width, w, s),
             bb = g4_extend(b.data.small.b, b.width, w, s);
    if (op <= 2) {
        if (ab | bb)
            return llg_gmp_sv4_x(w, s);
        return g4_small(op == 0 ? aa + ba : op == 1 ? aa - ba : aa * ba, 0, w, s);
    }
    uint64_t known, ones, unknown;
    if (op == 3) {
        known = (~aa & ~ab) | (~ba & ~bb);
        ones = aa & ~ab & ba & ~bb;
        unknown = ~(known | ones);
    } else if (op == 4) {
        ones = (aa & ~ab) | (ba & ~bb);
        known = ~aa & ~ab & ~ba & ~bb;
        unknown = ~(known | ones);
    } else {
        unknown = ab | bb;
        ones = (op == 5 ? aa ^ ba : ~(aa ^ ba)) & ~unknown;
    }
    return g4_small(ones | unknown, unknown, w, s);
}
static inline g4_t llg_gmp_sv4_neg(g4_t v) {
    if (v.width > 64)
        return llg_gmp_sv4_unary_wide(v, 0);
    return v.data.small.b ? llg_gmp_sv4_x(v.width, v.is_signed)
                          : g4_small(UINT64_C(0) - v.data.small.a, 0, v.width, v.is_signed);
}
static inline g4_t llg_gmp_sv4_bitneg(g4_t v) {
    if (v.width > 64)
        return llg_gmp_sv4_unary_wide(v, 1);
    return g4_small(~v.data.small.a | v.data.small.b, v.data.small.b, v.width, v.is_signed);
}
static inline g4_t g4_predicate(int truth) { return llg_gmp_sv4_fill((uint8_t)truth, 1, 0); }
static inline g4_t llg_gmp_sv4_lognot(g4_t v) {
    int t = g4_truth(v);
    return g4_predicate(t == 2 ? 2 : !t);
}
static inline g4_t llg_gmp_sv4_logand(g4_t a, g4_t b) {
    int x = g4_truth(a), y = g4_truth(b);
    return g4_predicate(!x || !y ? 0 : x == 1 && y == 1 ? 1 : 2);
}
static inline g4_t llg_gmp_sv4_logor(g4_t a, g4_t b) {
    int x = g4_truth(a), y = g4_truth(b);
    return g4_predicate(x == 1 || y == 1 ? 1 : !x && !y ? 0 : 2);
}
/* Compare op: 0 equality, 1 case equality, 2..5 ordered predicates. */
static inline g4_t g4_compare(g4_t a, g4_t b, unsigned op) {
    uint32_t w = g4_maxw(a, b);
    int s = a.is_signed && b.is_signed;
    if (w > 64)
        return llg_gmp_sv4_compare_wide(a, b, op);
    uint64_t aa = g4_extend(a.data.small.a, a.width, w, s),
             ab = g4_extend(a.data.small.b, a.width, w, s);
    uint64_t ba = g4_extend(b.data.small.a, b.width, w, s),
             bb = g4_extend(b.data.small.b, b.width, w, s);
    if (op == 1)
        return g4_predicate(aa == ba && ab == bb);
    if (op == 0)
        return g4_predicate(((aa ^ ba) & ~(ab | bb)) ? 0 : (ab | bb) ? 2 : 1);
    if (ab | bb)
        return g4_predicate(2);
    int c = aa < ba ? -1 : aa > ba ? 1 : 0;
    if (s && w) {
        int an = (int)(aa >> (w - 1)), bn = (int)(ba >> (w - 1));
        if (an != bn)
            c = an ? -1 : 1;
    }
    return g4_predicate(op == 2 ? c < 0 : op == 3 ? c <= 0 : op == 4 ? c > 0 : c >= 0);
}
static inline int llg_gmp_sv4_same(g4_t a, g4_t b) {
    return g4_maxw(a, b) <= 64
               ? a.data.small.a == b.data.small.a && a.data.small.b == b.data.small.b
               : llg_gmp_sv4_same_wide(a, b);
}
static inline g4_t llg_gmp_sv4_mux(g4_t sel, g4_t a, g4_t b) {
    uint32_t w = g4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    int t = g4_truth(sel);
    if (t != 2)
        return llg_gmp_sv4_resize(t ? a : b, w, s);
    if (w > 64)
        return llg_gmp_sv4_mux_wide(a, b);
    uint64_t aa = g4_extend(a.data.small.a, a.width, w, s),
             ab = g4_extend(a.data.small.b, a.width, w, s);
    uint64_t ba = g4_extend(b.data.small.a, b.width, w, s),
             bb = g4_extend(b.data.small.b, b.width, w, s);
    uint64_t unknown = (aa ^ ba) | ab | bb;
    return g4_small((aa & ~unknown) | unknown, unknown, w, s);
}
static inline g4_t g4_divmod(g4_t a, g4_t b, int remainder) {
    uint32_t w = g4_maxw(a, b);
    int8_t s = a.is_signed && b.is_signed;
    if (w > 64)
        return llg_gmp_sv4_divmod_wide(a, b, remainder);
    if (a.data.small.b | b.data.small.b)
        return llg_gmp_sv4_x(w, s);
    uint64_t x = g4_extend(a.data.small.a, a.width, w, s),
             y = g4_extend(b.data.small.a, b.width, w, s);
    if (!y)
        return llg_gmp_sv4_x(w, s);
    int xn = s && w && ((x >> (w - 1)) & 1), yn = s && w && ((y >> (w - 1)) & 1);
    if (xn)
        x = (UINT64_C(0) - x) & g4_mask(w);
    if (yn)
        y = (UINT64_C(0) - y) & g4_mask(w);
    uint64_t r = remainder ? x % y : x / y;
    if (remainder ? xn : xn != yn)
        r = UINT64_C(0) - r;
    return g4_small(r, 0, w, s);
}
static inline g4_t llg_gmp_sv4_div(g4_t a, g4_t b) { return g4_divmod(a, b, 0); }
static inline g4_t llg_gmp_sv4_mod(g4_t a, g4_t b) { return g4_divmod(a, b, 1); }
static inline g4_t llg_gmp_sv4_pow(g4_t a, g4_t b) {
    if (a.width > 64 || b.width > 64)
        return llg_gmp_sv4_pow_wide(a, b);
    if (a.data.small.b | b.data.small.b)
        return llg_gmp_sv4_x(a.width, a.is_signed);
    uint64_t base = a.data.small.a, exponent = b.data.small.a;
    if (b.is_signed && b.width && ((exponent >> (b.width - 1)) & 1)) {
        if (!base)
            return llg_gmp_sv4_x(a.width, a.is_signed);
        if (a.is_signed && base == g4_mask(a.width))
            return llg_gmp_sv4_from_u64(exponent & 1 ? base : 1, a.width, a.is_signed);
        return llg_gmp_sv4_from_u64(base == 1 ? 1 : 0, a.width, a.is_signed);
    }
    uint64_t r = 1;
    while (exponent) {
        if (exponent & 1)
            r *= base;
        exponent >>= 1;
        if (exponent)
            base *= base;
    }
    return g4_small(r, 0, a.width, a.is_signed);
}
static inline g4_t llg_gmp_sv4_clog2(g4_t v) {
    if (v.width > 64)
        return llg_gmp_sv4_clog2_wide(v);
    if (v.data.small.b)
        return llg_gmp_sv4_x(32, 0);
    uint64_t n = v.data.small.a;
    unsigned r = 0;
    if (n)
        --n;
    while (n) {
        ++r;
        n >>= 1;
    }
    return g4_small(r, 0, 32, 0);
}
static inline llg_gmp_sv4_vpi_word_t llg_gmp_sv4_vpi_word(g4_t v, size_t word) {
    llg_gmp_sv4_vpi_word_t r = {0, 0};
    if (word >= llg_gmp_sv4_words(v))
        return r;
    if (v.width <= 64) {
        r.aval = v.data.small.a;
        r.bval = v.data.small.b;
    } else {
        r.aval = v.data.wide.a[word];
        r.bval = v.data.wide.b ? v.data.wide.b[word] : 0;
    }
    return r;
}
static inline uint64_t llg_gmp_sv4_word(g4_t v, size_t word, unsigned plane) {
    llg_gmp_sv4_vpi_word_t r = llg_gmp_sv4_vpi_word(v, word);
    return plane == 0 ? r.aval & ~r.bval : plane == 1 ? r.aval & r.bval : ~r.aval & r.bval;
}
/* Internal zero-padded logical-plane slice, count in 1..64. */
static inline uint64_t llg_gmp_sv4_plane_slice(g4_t value, uint32_t low,
                                             unsigned count, unsigned plane) {
    size_t word = low / 64u;
    unsigned shift = low % 64u;
    uint64_t result = llg_gmp_sv4_word(value, word, plane) >> shift;
    if (shift && count > 64u - shift)
        result |= llg_gmp_sv4_word(value, word + 1u, plane) << (64u - shift);
    return result & g4_mask(count);
}
static inline unsigned llg_gmp_sv4_state(g4_t v, uint64_t bit) {
    if (bit >= v.width)
        return 2;
    llg_gmp_sv4_vpi_word_t r = llg_gmp_sv4_vpi_word(v, (size_t)(bit / 64));
    unsigned a = (unsigned)((r.aval >> (bit % 64)) & 1), b = (unsigned)((r.bval >> (bit % 64)) & 1);
    return b ? (a ? 2u : 3u) : a;
}
static inline void llg_gmp_sv4_set_vpi_word(g4_t* v, size_t word, llg_gmp_sv4_vpi_word_t input) {
    if (word >= llg_gmp_sv4_words(*v))
        return;
    if (v->width <= 64) {
        v->data.small.a = input.aval & g4_mask(v->width);
        v->data.small.b = input.bval & g4_mask(v->width);
    } else
        llg_gmp_sv4_set_ab_wide(v, word, input.aval, input.bval);
}
static inline void llg_gmp_sv4_set_word(g4_t* v, size_t word, uint64_t bits, uint64_t x,
                                        uint64_t z) {
    llg_gmp_sv4_vpi_word_t r = {(bits & ~(x | z)) | x, x | z};
    llg_gmp_sv4_set_vpi_word(v, word, r);
}
static inline void llg_gmp_sv4_set_state(g4_t* v, uint64_t bit, unsigned state) {
    if (state > 3)
        llg_gmp_sv4_fail("invalid scalar state");
    if (bit >= v->width)
        return;
    size_t word = (size_t)(bit / 64);
    uint64_t mask = UINT64_C(1) << (bit % 64);
    llg_gmp_sv4_vpi_word_t r = llg_gmp_sv4_vpi_word(*v, word);
    r.aval = (r.aval & ~mask) | (state == 1 || state == 2 ? mask : 0);
    r.bval = (r.bval & ~mask) | (state >= 2 ? mask : 0);
    llg_gmp_sv4_set_vpi_word(v, word, r);
}
static inline int llg_gmp_sv4_has_x(g4_t v) {
    return v.width <= 64 ? (v.data.small.a & v.data.small.b) != 0
                         : llg_gmp_sv4_has_plane_wide(v, 0);
}
static inline int llg_gmp_sv4_has_z(g4_t v) {
    return v.width <= 64 ? (~v.data.small.a & v.data.small.b) != 0
                         : llg_gmp_sv4_has_plane_wide(v, 1);
}
static inline unsigned llg_gmp_sv4_state_to_dpi(unsigned state) {
    return state < 2 ? state : state ^ 1u;
}
static inline unsigned llg_gmp_sv4_state_from_dpi(unsigned state) {
    return state < 2 ? state : state ^ 1u;
}
static inline size_t llg_gmp_sv4_word_range(g4_t v, size_t first, size_t count) {
    size_t n = llg_gmp_sv4_words(v);
    return first >= n ? 0 : count < n - first ? count : n - first;
}
void llg_gmp_sv4_export_words_wide(g4_t v, size_t first, llg_gmp_sv4_word_t* output, size_t count);
void llg_gmp_sv4_export_vpi_words_wide(g4_t v, size_t first, llg_gmp_sv4_vpi_word_t* output,
                                       size_t count);
static inline g4_t llg_gmp_sv4_add(g4_t a, g4_t b) { return g4_binary(a, b, 0); }
static inline g4_t llg_gmp_sv4_sub(g4_t a, g4_t b) { return g4_binary(a, b, 1); }
static inline g4_t llg_gmp_sv4_mul(g4_t a, g4_t b) { return g4_binary(a, b, 2); }
void llg_gmp_sv4_arithmetic_into_wide(g4_t* dst, g4_t a, g4_t b, unsigned op);
/* Borrow operands; exact destination aliases are supported. */
static inline void g4_arithmetic_into(g4_t* dst, g4_t a, g4_t b, unsigned op) {
    if (g4_maxw(a, b) <= 64)
        llg_gmp_sv4_replace(dst, g4_binary(a, b, op));
    else
        llg_gmp_sv4_arithmetic_into_wide(dst, a, b, op);
}
static inline void llg_gmp_sv4_add_into(g4_t* dst, g4_t a, g4_t b) {
    g4_arithmetic_into(dst, a, b, 0);
}
static inline void llg_gmp_sv4_sub_into(g4_t* dst, g4_t a, g4_t b) {
    g4_arithmetic_into(dst, a, b, 1);
}
static inline void llg_gmp_sv4_mul_into(g4_t* dst, g4_t a, g4_t b) {
    g4_arithmetic_into(dst, a, b, 2);
}
static inline g4_t llg_gmp_sv4_and(g4_t a, g4_t b) { return g4_binary(a, b, 3); }
static inline g4_t llg_gmp_sv4_or(g4_t a, g4_t b) { return g4_binary(a, b, 4); }
static inline g4_t llg_gmp_sv4_xor(g4_t a, g4_t b) { return g4_binary(a, b, 5); }
static inline g4_t llg_gmp_sv4_xnor(g4_t a, g4_t b) { return g4_binary(a, b, 6); }
static inline g4_t llg_gmp_sv4_eq(g4_t a, g4_t b) { return g4_compare(a, b, 0); }
static inline g4_t llg_gmp_sv4_case_eq(g4_t a, g4_t b) { return g4_compare(a, b, 1); }
static inline g4_t llg_gmp_sv4_lt(g4_t a, g4_t b) { return g4_compare(a, b, 2); }
static inline g4_t llg_gmp_sv4_le(g4_t a, g4_t b) { return g4_compare(a, b, 3); }
static inline g4_t llg_gmp_sv4_gt(g4_t a, g4_t b) { return g4_compare(a, b, 4); }
static inline g4_t llg_gmp_sv4_ge(g4_t a, g4_t b) { return g4_compare(a, b, 5); }
static inline g4_t llg_gmp_sv4_neq(g4_t a, g4_t b) {
    return llg_gmp_sv4_lognot(llg_gmp_sv4_eq(a, b));
}
static inline g4_t llg_gmp_sv4_case_neq(g4_t a, g4_t b) {
    return llg_gmp_sv4_lognot(llg_gmp_sv4_case_eq(a, b));
}
/* V05/S2: shifts and reductions. Counts are unsigned bit patterns. */
g4_t llg_gmp_sv4_shift_wide(g4_t value, g4_t count, int right, int arithmetic);
g4_t llg_gmp_sv4_reduce_wide(g4_t value, unsigned op);
g4_t llg_gmp_sv4_countones_wide(g4_t value);
g4_t llg_gmp_sv4_onehot_wide(g4_t value, int allow_zero);
static inline unsigned g4_popcount(uint64_t x) {
    x -= (x >> 1) & UINT64_C(0x5555555555555555);
    x = (x & UINT64_C(0x3333333333333333)) + ((x >> 2) & UINT64_C(0x3333333333333333));
    x = (x + (x >> 4)) & UINT64_C(0x0f0f0f0f0f0f0f0f);
    return (unsigned)((x * UINT64_C(0x0101010101010101)) >> 56);
}
static inline g4_t g4_shift(g4_t v, g4_t count, int right, int arithmetic) {
    if (v.width > 64 || count.width > 64)
        return llg_gmp_sv4_shift_wide(v, count, right, arithmetic);
    if (count.data.small.b)
        return llg_gmp_sv4_x(v.width, v.is_signed);
    uint64_t sh = count.data.small.a;
    uint64_t a = v.data.small.a, b = v.data.small.b;
    unsigned top = v.width ? v.width - 1u : 0;
    uint64_t fill_a =
        right && arithmetic && v.is_signed && v.width && ((a >> top) & 1) ? UINT64_MAX : 0;
    uint64_t fill_b =
        right && arithmetic && v.is_signed && v.width && ((b >> top) & 1) ? UINT64_MAX : 0;
    if (sh >= v.width)
        return g4_small(fill_a, fill_b, v.width, v.is_signed);
    if (!sh)
        return v;
    if (right) {
        uint64_t pad = ~g4_mask(v.width - (uint32_t)sh);
        return g4_small((a >> sh) | (fill_a & pad), (b >> sh) | (fill_b & pad), v.width,
                        v.is_signed);
    }
    return g4_small(a << sh, b << sh, v.width, v.is_signed);
}
static inline g4_t llg_gmp_sv4_shl(g4_t a, g4_t b) { return g4_shift(a, b, 0, 0); }
static inline g4_t llg_gmp_sv4_shr(g4_t a, g4_t b) { return g4_shift(a, b, 1, 0); }
static inline g4_t llg_gmp_sv4_ashl(g4_t a, g4_t b) { return g4_shift(a, b, 0, 1); }
static inline g4_t llg_gmp_sv4_ashr(g4_t a, g4_t b) { return g4_shift(a, b, 1, 1); }
static inline g4_t g4_reduce(g4_t v, unsigned op) {
    if (v.width > 64)
        return llg_gmp_sv4_reduce_wide(v, op);
    uint64_t a = v.data.small.a, b = v.data.small.b;
    unsigned kind = op % 3u;
    int t;
    if (kind == 0)
        t = (~a & ~b & g4_mask(v.width)) ? 0 : b ? 2 : 1;
    else if (kind == 1)
        t = (a & ~b) ? 1 : b ? 2 : 0;
    else
        t = b ? 2 : (int)(g4_popcount(a) & 1u);
    return g4_predicate(op >= 3 && t != 2 ? !t : t);
}
static inline g4_t llg_gmp_sv4_reduce_and(g4_t v) { return g4_reduce(v, 0); }
static inline g4_t llg_gmp_sv4_reduce_or(g4_t v) { return g4_reduce(v, 1); }
static inline g4_t llg_gmp_sv4_reduce_xor(g4_t v) { return g4_reduce(v, 2); }
static inline g4_t llg_gmp_sv4_reduce_nand(g4_t v) { return g4_reduce(v, 3); }
static inline g4_t llg_gmp_sv4_reduce_nor(g4_t v) { return g4_reduce(v, 4); }
static inline g4_t llg_gmp_sv4_reduce_xnor(g4_t v) { return g4_reduce(v, 5); }
static inline g4_t llg_gmp_sv4_countones(g4_t v) {
    return v.width > 64 ? llg_gmp_sv4_countones_wide(v)
                        : g4_small(g4_popcount(v.data.small.a & ~v.data.small.b), 0, 32, 1);
}
static inline g4_t llg_gmp_sv4_onehot(g4_t v, int allow_zero) {
    if (v.width > 64)
        return llg_gmp_sv4_onehot_wide(v, allow_zero);
    uint64_t ones = v.data.small.a & ~v.data.small.b;
    return g4_predicate(ones ? !(ones & (ones - 1u)) : allow_zero != 0);
}
/* End V05/S2. */

/* V05/S3: case modes, directional wildcards and range membership. */
g4_t llg_gmp_sv4_match_wide(g4_t a, g4_t b, unsigned mode);
static inline g4_t g4_match(g4_t a, g4_t b, unsigned mode) {
    if (g4_maxw(a, b) > 64)
        return llg_gmp_sv4_match_wide(a, b, mode);
    uint32_t w = g4_maxw(a, b);
    int sign = mode == 2 && a.is_signed && b.is_signed;
    uint64_t aa = g4_extend(a.data.small.a, a.width, w, sign);
    uint64_t ab = g4_extend(a.data.small.b, a.width, w, sign);
    uint64_t ba = g4_extend(b.data.small.a, b.width, w, sign);
    uint64_t bb = g4_extend(b.data.small.b, b.width, w, sign);
    if (mode == 0)
        return g4_predicate(!((aa ^ ba) & ~(ab | bb)));
    if (mode == 1) {
        uint64_t care = ~((~aa & ab) | (~ba & bb)) & g4_mask(w);
        return g4_predicate(!(((aa ^ ba) | (ab ^ bb)) & care));
    }
    uint64_t care = ~bb & g4_mask(w);
    if ((aa ^ ba) & ~ab & care)
        return g4_predicate(0);
    return g4_predicate(ab & care ? 2 : 1);
}
static inline g4_t llg_gmp_sv4_casex_eq(g4_t a, g4_t b) { return g4_match(a, b, 0); }
static inline g4_t llg_gmp_sv4_casez_eq(g4_t a, g4_t b) { return g4_match(a, b, 1); }
static inline g4_t llg_gmp_sv4_wild_eq(g4_t a, g4_t b) { return g4_match(a, b, 2); }
static inline g4_t llg_gmp_sv4_wild_neq(g4_t a, g4_t b) {
    return llg_gmp_sv4_lognot(llg_gmp_sv4_wild_eq(a, b));
}
static inline g4_t llg_gmp_sv4_logimpl(g4_t a, g4_t b) {
    int x = g4_truth(a), y = g4_truth(b);
    return g4_predicate(x == 0 || y == 1 ? 1 : x == 1 && y == 0 ? 0 : 2);
}
static inline g4_t llg_gmp_sv4_logequiv(g4_t a, g4_t b) {
    int x = g4_truth(a), y = g4_truth(b);
    return g4_predicate(x == 2 || y == 2 ? 2 : x == y);
}
static inline g4_t llg_gmp_sv4_inside_range(g4_t value, g4_t low, g4_t high) {
    g4_t ge = llg_gmp_sv4_ge(value, low), le = llg_gmp_sv4_le(value, high);
    return llg_gmp_sv4_logand(ge, le);
}
/* End V05/S3. */

static inline void llg_gmp_sv4_export_vpi_words(g4_t v, size_t first, llg_gmp_sv4_vpi_word_t* out,
                                                size_t count) {
    if (v.width > 64) {
        llg_gmp_sv4_export_vpi_words_wide(v, first, out, count);
        return;
    }
    size_t m = llg_gmp_sv4_word_range(v, first, count);
    for (size_t i = 0; i < count; ++i) {
        llg_gmp_sv4_vpi_word_t r = {0, 0};
        if (i < m) {
            r.aval = v.data.small.a;
            r.bval = v.data.small.b;
        }
        out[i] = r;
    }
}
static inline void llg_gmp_sv4_export_words(g4_t v, size_t first, llg_gmp_sv4_word_t* out,
                                            size_t count) {
    if (v.width > 64) {
        llg_gmp_sv4_export_words_wide(v, first, out, count);
        return;
    }
    size_t m = llg_gmp_sv4_word_range(v, first, count);
    for (size_t i = 0; i < count; ++i) {
        llg_gmp_sv4_word_t r = {0, 0, 0};
        if (i < m) {
            r.bits = v.data.small.a & ~v.data.small.b;
            r.x = v.data.small.a & v.data.small.b;
            r.z = ~v.data.small.a & v.data.small.b;
        }
        out[i] = r;
    }
}
static inline void llg_gmp_sv4_import_vpi_words(g4_t* v, size_t first,
                                                const llg_gmp_sv4_vpi_word_t* in, size_t count) {
    if (v->width > 64) {
        llg_gmp_sv4_import_vpi_words_wide(v, first, in, count);
        return;
    }
    if (llg_gmp_sv4_word_range(*v, first, count))
        llg_gmp_sv4_set_vpi_word(v, 0, in[0]);
}
static inline void llg_gmp_sv4_import_words(g4_t* v, size_t first, const llg_gmp_sv4_word_t* in,
                                            size_t count) {
    if (v->width > 64) {
        llg_gmp_sv4_import_words_wide(v, first, in, count);
        return;
    }
    if (llg_gmp_sv4_word_range(*v, first, count))
        llg_gmp_sv4_set_word(v, 0, in[0].bits, in[0].x, in[0].z);
}

#include "selection_inline.h"
#include "consumer_inline.h"

/* V05/S6: net metadata remains outside packed storage. Inputs borrow;
 * returned values own. UDP rows and scalar inputs are validated by lowering. */
enum {
    LLG_GMP_RESOLVE_WIRE = 0, LLG_GMP_RESOLVE_WAND = 1, LLG_GMP_RESOLVE_WOR = 2,
    LLG_GMP_RESOLVE_TRI0 = 3, LLG_GMP_RESOLVE_TRI1 = 4,
    LLG_GMP_RESOLVE_SUPPLY0 = 5, LLG_GMP_RESOLVE_SUPPLY1 = 6
};
enum {
    LLG_GMP_STRENGTH_HIGHZ = 0, LLG_GMP_STRENGTH_SMALL = 1, LLG_GMP_STRENGTH_MEDIUM = 2,
    LLG_GMP_STRENGTH_WEAK = 3, LLG_GMP_STRENGTH_LARGE = 4, LLG_GMP_STRENGTH_PULL = 5,
    LLG_GMP_STRENGTH_STRONG = 6, LLG_GMP_STRENGTH_SUPPLY = 7
};
g4_t llg_gmp_sv4_enum_navigate(g4_t current, g4_t step, const g4_t* values,
                                uint32_t count, g4_t default_value, int direction);
g4_t llg_gmp_sv4_resolve(const g4_t* const* drivers, int count, uint32_t width,
                          int8_t sign, int mode);
g4_t llg_gmp_sv4_resolve_strengths(const g4_t* const* drivers, const uint8_t* strength0,
                                    const uint8_t* strength1, int count, uint32_t width,
                                    int8_t sign, int mode);
g4_t llg_gmp_sv4_resolve_strengths_range(const g4_t* const* drivers, const uint8_t* strength0,
                                          const uint8_t* strength1, const int* indices, int count,
                                          uint32_t source_width, uint32_t offset, uint32_t width,
                                          int8_t sign, int mode);
g4_t llg_gmp_sv4_udp_eval(const uint8_t* rows, size_t row_count, size_t input_count,
                           const g4_t* const* inputs);
/* End V05/S6. */

#ifndef LLG_SV4_DECIMAL_GMP_THRESHOLD
#define LLG_SV4_DECIMAL_GMP_THRESHOLD 4u
#endif
/* V05/S8: low-bit scalar coercion versus exact host index conversion.
 * Formatting borrows values and writes only a bounded NUL-terminated prefix. */
int llg_gmp_sv4_fits_i64_wide(g4_t value);
uint64_t llg_gmp_sv4_to_index_wide(g4_t value);
void llg_gmp_sv4_to_dec_string_wide(g4_t value, char* buf, size_t cap);
void llg_gmp_sv4_format(char fmt, g4_t value, char* buf, size_t cap);
static inline int64_t llg_gmp_sv4_to_i64(g4_t value) {
    uint64_t bits = llg_gmp_sv4_to_u64(value);
    if (value.width && value.width < 64 && ((bits >> (value.width - 1u)) & 1u))
        bits |= ~g4_mask(value.width);
    return bits <= INT64_MAX ? (int64_t)bits : -1 - (int64_t)~bits;
}
static inline int llg_gmp_sv4_fits_i64(g4_t value) {
    if (value.width > 64)
        return llg_gmp_sv4_fits_i64_wide(value);
    return !value.data.small.b &&
           (value.is_signed || value.width < 64 || !(value.data.small.a >> 63));
}
static inline uint64_t llg_gmp_sv4_to_index(g4_t value) {
    if (value.width > 64)
        return llg_gmp_sv4_to_index_wide(value);
    if (value.data.small.b || (value.is_signed && value.width &&
        ((value.data.small.a >> (value.width - 1u)) & 1u)))
        return UINT64_MAX;
    return value.data.small.a;
}
static inline int llg_gmp_sv4_to_index_i64(g4_t value, int64_t* out) {
    if (!out || !llg_gmp_sv4_fits_i64(value))
        return 0;
    *out = value.is_signed ? llg_gmp_sv4_to_i64(value) : (int64_t)llg_gmp_sv4_to_u64(value);
    return 1;
}
static inline void llg_gmp_sv4_to_dec_string(g4_t value, char* buf, size_t cap) {
    if (!cap)
        return;
    if (value.width > 64) {
        llg_gmp_sv4_to_dec_string_wide(value, buf, cap);
        return;
    }
    size_t len = 0;
    if (value.data.small.b) {
        buf[0] = cap > 1 ? 'x' : 0;
        if (cap > 1)
            buf[1] = 0;
        return;
    }
    uint64_t bits = value.data.small.a;
    int negative = value.is_signed && value.width && ((bits >> (value.width - 1u)) & 1u);
    if (negative) {
        bits = (UINT64_C(0) - bits) & g4_mask(value.width);
        if (len + 1u < cap)
            buf[len++] = '-';
    }
    char digits[20];
    size_t count = 0;
    do {
        digits[count++] = (char)('0' + bits % 10u);
        bits /= 10u;
    } while (bits);
    while (count && len + 1u < cap)
        buf[len++] = digits[--count];
    buf[len] = 0;
}
/* End V05/S8. */

/* V05/S7: native real payloads and scheduler ticks never enter packed storage.
 * Casts round ties away from zero; rtoi truncates. X/Z bits contribute zero
 * to real inspectors. Nonfinite integer conversions yield X. */
double llg_gmp_sv4_to_real_wide(g4_t value);
g4_t llg_gmp_sv4_from_real_wide(double rounded, uint32_t width, int8_t sign);
uint64_t llg_gmp_sv4_delay_ticks(g4_t value, uint64_t unit_ticks);
uint64_t llg_gmp_sv4_real_delay_ticks(double value, uint64_t unit_ticks, uint64_t precision_ticks);
static inline double llg_gmp_sv4_to_real(g4_t value) {
    if (value.width > 64)
        return llg_gmp_sv4_to_real_wide(value);
    uint64_t bits = value.data.small.a & ~value.data.small.b;
    int negative = value.is_signed && value.width && ((bits >> (value.width - 1u)) & 1u);
    uint64_t magnitude = negative ? (UINT64_C(0) - bits) & g4_mask(value.width) : bits;
    return negative ? -(double)magnitude : (double)magnitude;
}
static inline g4_t llg_gmp_sv4_from_real(double value, uint32_t width, int8_t sign) {
    g4_width_check(width);
    if (!isfinite(value))
        return llg_gmp_sv4_x(width, sign);
    double rounded = round(value);
    if (width > 64)
        return llg_gmp_sv4_from_real_wide(rounded, width, sign);
    uint64_t bits = (uint64_t)fmod(fabs(rounded), 18446744073709551616.0);
    if (signbit(rounded))
        bits = UINT64_C(0) - bits;
    return g4_small(bits, 0, width, sign);
}
static inline g4_t llg_gmp_sv4_rtoi(double value) {
    if (!isfinite(value))
        return llg_gmp_sv4_x(32, 1);
    uint64_t bits = (uint64_t)fmod(fabs(trunc(value)), 4294967296.0);
    return g4_small(signbit(value) ? UINT64_C(0) - bits : bits, 0, 32, 1);
}
static inline g4_t llg_gmp_sv4_realtobits(double value) {
    uint64_t bits;
    memcpy(&bits, &value, sizeof(bits));
    return g4_small(bits, 0, 64, 0);
}
static inline double llg_gmp_sv4_bitstoreal(g4_t value) {
    uint64_t bits = llg_gmp_sv4_to_u64(value);
    double out;
    memcpy(&out, &bits, sizeof(out));
    return out;
}
static inline g4_t llg_gmp_sv4_shortrealtobits(double value) {
    float rounded = (float)value;
    uint32_t bits;
    memcpy(&bits, &rounded, sizeof(bits));
    return g4_small(bits, 0, 32, 0);
}
static inline double llg_gmp_sv4_bitstoshortreal(g4_t value) {
    uint32_t bits = (uint32_t)llg_gmp_sv4_to_u64(value);
    float out;
    memcpy(&out, &bits, sizeof(out));
    return (double)out;
}
static inline int llg_gmp_real_to_bool(double value) { return value != 0.0; }
/* End V05/S7. */

/* V05/S9: checked widths and owner-free facade helper types. */
static inline uint32_t llg_gmp_sv4_checked_width(g4_t value) {
    if (llg_gmp_sv4_is_unknown(value) || (value.is_signed && value.width &&
        llg_gmp_sv4_state(value, value.width - 1u) == 1u))
        llg_gmp_sv4_fail("invalid dynamic packed width");
    uint64_t width = llg_gmp_sv4_to_index(value);
    if (width >= LLG_GMP_SUPPORTED_WIDTH_LIMIT)
        llg_gmp_sv4_fail("width reaches exclusive limit");
    return (uint32_t)width;
}
/* End V05/S9. */

#ifdef __cplusplus
}
#endif

/* Standalone facade. V07 owns selection through llg_value.h. */
#ifdef LLG_SV4_GMP_PUBLIC_NAMES
/* V05d consumer aliases. */
#define llg_sv4_masked_merge llg_gmp_sv4_masked_merge
#define llg_sv4_masked_copy llg_gmp_sv4_masked_copy
#define llg_sv4_masked_same llg_gmp_sv4_masked_same
#define llg_sv4_range_same llg_gmp_sv4_range_same
#define llg_sv4_range_copy llg_gmp_sv4_range_copy
#define llg_sv4_range_fill llg_gmp_sv4_range_fill
#define llg_sv4_mask_remove llg_gmp_sv4_mask_remove
#define llg_sv4_mask_top llg_gmp_sv4_mask_top
#define llg_sv4_mul_add_known llg_gmp_sv4_mul_add_known
#define llg_sv4_negate_known llg_gmp_sv4_negate_known
#define llg_sv4_two_state_inplace llg_gmp_sv4_two_state_inplace
#define llg_sv4_append_digit llg_gmp_sv4_append_digit
#define llg_sv4_export_vpi32 llg_gmp_sv4_export_vpi32
#define llg_sv4_import_vpi32 llg_gmp_sv4_import_vpi32
#define llg_sv4_same_vpi_words llg_gmp_sv4_same_vpi_words
#define llg_sv4_export_text llg_gmp_sv4_export_text
#define llg_sv4_plane_slice llg_gmp_sv4_plane_slice

/* V05/S2 and S3 public names. */
#define sv4_ashl llg_gmp_sv4_ashl
#define sv4_ashr llg_gmp_sv4_ashr
#define sv4_shl llg_gmp_sv4_shl
#define sv4_shr llg_gmp_sv4_shr
#define sv4_countones llg_gmp_sv4_countones
#define sv4_onehot llg_gmp_sv4_onehot
#define sv4_reduce_and llg_gmp_sv4_reduce_and
#define sv4_reduce_nand llg_gmp_sv4_reduce_nand
#define sv4_reduce_nor llg_gmp_sv4_reduce_nor
#define sv4_reduce_or llg_gmp_sv4_reduce_or
#define sv4_reduce_xnor llg_gmp_sv4_reduce_xnor
#define sv4_reduce_xor llg_gmp_sv4_reduce_xor
#define sv4_casex_eq llg_gmp_sv4_casex_eq
#define sv4_casez_eq llg_gmp_sv4_casez_eq
#define sv4_inside_range llg_gmp_sv4_inside_range
#define sv4_logequiv llg_gmp_sv4_logequiv
#define sv4_logimpl llg_gmp_sv4_logimpl
#define sv4_wild_eq llg_gmp_sv4_wild_eq
#define sv4_wild_neq llg_gmp_sv4_wild_neq
/* End V05/S2 and S3 public names. */


/* V05 S4/S5 public names. */
#define sv4_bit_select llg_gmp_sv4_bit_select
#define sv4_bit_select_set llg_gmp_sv4_bit_select_set
#define sv4_part_select llg_gmp_sv4_part_select
#define sv4_part_select_set llg_gmp_sv4_part_select_set
#define sv4_idx_part_select llg_gmp_sv4_idx_part_select
#define sv4_idx_part_select_set llg_gmp_sv4_idx_part_select_set
#define sv4_idx_part_select_value llg_gmp_sv4_idx_part_select_value
#define sv4_idx_part_select_set_value llg_gmp_sv4_idx_part_select_set_value
#define sv4_select_plan_init llg_gmp_sv4_select_plan_init
#define sv4_select_plan_bit llg_gmp_sv4_select_plan_bit
#define sv4_select_plan_part llg_gmp_sv4_select_plan_part
#define sv4_select_plan_indexed llg_gmp_sv4_select_plan_indexed
#define sv4_select_plan_step llg_gmp_sv4_select_plan_step
#define sv4_select_plan_read llg_gmp_sv4_select_plan_read
#define sv4_select_plan_slice llg_gmp_sv4_select_plan_slice
#define sv4_select_plan_set llg_gmp_sv4_select_plan_set
#define sv4_concat llg_gmp_sv4_concat
#define sv4_repeat llg_gmp_sv4_repeat
#define sv4_repeat_count llg_gmp_sv4_repeat_count
#define sv4_stream llg_gmp_sv4_stream
#define sv4_unstream llg_gmp_sv4_unstream
#define sv4_array_conditional_merge llg_gmp_sv4_array_conditional_merge
#define llg_ref_read llg_gmp_ref_read
#define llg_ref_view_valid llg_gmp_ref_view_valid
#define sv4_select_plan_t llg_gmp_sv4_select_plan_t
#define llg_queue_ref_read_fn llg_gmp_queue_ref_read_fn
#define llg_queue_ref_write_fn llg_gmp_queue_ref_write_fn
#define llg_ref_kind_t llg_gmp_ref_kind_t
#define llg_ref_t llg_gmp_ref_t
#define llg_ref_composite_t llg_gmp_ref_composite_t
#define llg_ref_tag_check_t llg_gmp_ref_tag_check_t
#define llg_ref_view_t llg_gmp_ref_view_t
#define LLG_REF_WHOLE LLG_GMP_REF_WHOLE
#define LLG_REF_BIT LLG_GMP_REF_BIT
#define LLG_REF_PART LLG_GMP_REF_PART
#define LLG_REF_INDEXED LLG_GMP_REF_INDEXED
#define LLG_REF_ARRAY LLG_GMP_REF_ARRAY
#define LLG_REF_QUEUE LLG_GMP_REF_QUEUE
#define LLG_REF_PACKED_PLAN LLG_GMP_REF_PACKED_PLAN
#define LLG_REF_COMPOSITE LLG_GMP_REF_COMPOSITE
#define LLG_REF_VIEW LLG_GMP_REF_VIEW
#define LLG_REF_TAGGED_VIEW LLG_GMP_REF_TAGGED_VIEW
#define sv4_add llg_gmp_sv4_add
#define sv4_add_into llg_gmp_sv4_add_into
#define sv4_and llg_gmp_sv4_and
#define sv4_assign llg_gmp_sv4_assign
#define sv4_bitneg llg_gmp_sv4_bitneg
#define sv4_bytes llg_gmp_sv4_bytes
#define sv4_case_eq llg_gmp_sv4_case_eq
#define sv4_case_neq llg_gmp_sv4_case_neq
#define sv4_cast llg_gmp_sv4_cast
#define sv4_clog2 llg_gmp_sv4_clog2
#define sv4_clone llg_gmp_sv4_clone
#define sv4_copy llg_gmp_sv4_copy
#define sv4_destroy llg_gmp_sv4_destroy
#define sv4_destroy_array llg_gmp_sv4_destroy_array
#define sv4_div llg_gmp_sv4_div
#define sv4_eq llg_gmp_sv4_eq
#define llg_sv4_export_vpi_words llg_gmp_sv4_export_vpi_words
#define llg_sv4_export_words llg_gmp_sv4_export_words
#define sv4_fill llg_gmp_sv4_fill
#define sv4_from_i64 llg_gmp_sv4_from_i64
#define sv4_from_limbs llg_gmp_sv4_from_limbs
#define sv4_from_masks llg_gmp_sv4_from_masks
#define sv4_from_u64 llg_gmp_sv4_from_u64
#define sv4_ge llg_gmp_sv4_ge
#define sv4_gt llg_gmp_sv4_gt
#define llg_sv4_has_x llg_gmp_sv4_has_x
#define llg_sv4_has_z llg_gmp_sv4_has_z
#define llg_sv4_import_vpi_words llg_gmp_sv4_import_vpi_words
#define llg_sv4_import_words llg_gmp_sv4_import_words
#define sv4_is_unknown llg_gmp_sv4_is_unknown
#define sv4_le llg_gmp_sv4_le
#define sv4_logand llg_gmp_sv4_logand
#define sv4_lognot llg_gmp_sv4_lognot
#define sv4_logor llg_gmp_sv4_logor
#define sv4_lt llg_gmp_sv4_lt
#define sv4_mod llg_gmp_sv4_mod
#define sv4_move llg_gmp_sv4_move
#define sv4_mul llg_gmp_sv4_mul
#define sv4_mul_into llg_gmp_sv4_mul_into
#define sv4_mux llg_gmp_sv4_mux
#define sv4_neg llg_gmp_sv4_neg
#define sv4_neq llg_gmp_sv4_neq
#define sv4_or llg_gmp_sv4_or
#define sv4_pow llg_gmp_sv4_pow
#define sv4_replace llg_gmp_sv4_replace
#define sv4_resize llg_gmp_sv4_resize
#define sv4_same llg_gmp_sv4_same
#define llg_sv4_set_signed llg_gmp_sv4_set_signed
#define llg_sv4_set_state llg_gmp_sv4_set_state
#define llg_sv4_set_vpi_word llg_gmp_sv4_set_vpi_word
#define llg_sv4_set_word llg_gmp_sv4_set_word
#define llg_sv4_signed llg_gmp_sv4_signed
#define llg_sv4_state llg_gmp_sv4_state
#define llg_sv4_state_from_dpi llg_gmp_sv4_state_from_dpi
#define llg_sv4_state_to_dpi llg_gmp_sv4_state_to_dpi
#define sv4_sub llg_gmp_sv4_sub
#define sv4_sub_into llg_gmp_sv4_sub_into
#define sv4_to_bool llg_gmp_sv4_to_bool
#define sv4_to_two_state llg_gmp_sv4_to_two_state
#define sv4_to_u64 llg_gmp_sv4_to_u64
#define llg_sv4_vpi_word llg_gmp_sv4_vpi_word
#define llg_sv4_width llg_gmp_sv4_width
#define llg_sv4_word llg_gmp_sv4_word
#define llg_sv4_word_range llg_gmp_sv4_word_range
#define llg_sv4_words llg_gmp_sv4_words
#define sv4_x llg_gmp_sv4_x
#define sv4_xnor llg_gmp_sv4_xnor
#define sv4_xor llg_gmp_sv4_xor
#define sv4_zero llg_gmp_sv4_zero
/* V05/S6 public names. */
#define sv4_enum_navigate llg_gmp_sv4_enum_navigate
#define sv4_resolve llg_gmp_sv4_resolve
#define sv4_resolve_strengths llg_gmp_sv4_resolve_strengths
#define sv4_resolve_strengths_range llg_gmp_sv4_resolve_strengths_range
#define sv4_udp_eval llg_gmp_sv4_udp_eval
#define LLG_RESOLVE_WIRE LLG_GMP_RESOLVE_WIRE
#define LLG_RESOLVE_WAND LLG_GMP_RESOLVE_WAND
#define LLG_RESOLVE_WOR LLG_GMP_RESOLVE_WOR
#define LLG_RESOLVE_TRI0 LLG_GMP_RESOLVE_TRI0
#define LLG_RESOLVE_TRI1 LLG_GMP_RESOLVE_TRI1
#define LLG_RESOLVE_SUPPLY0 LLG_GMP_RESOLVE_SUPPLY0
#define LLG_RESOLVE_SUPPLY1 LLG_GMP_RESOLVE_SUPPLY1
#define LLG_STRENGTH_HIGHZ LLG_GMP_STRENGTH_HIGHZ
#define LLG_STRENGTH_SMALL LLG_GMP_STRENGTH_SMALL
#define LLG_STRENGTH_MEDIUM LLG_GMP_STRENGTH_MEDIUM
#define LLG_STRENGTH_WEAK LLG_GMP_STRENGTH_WEAK
#define LLG_STRENGTH_LARGE LLG_GMP_STRENGTH_LARGE
#define LLG_STRENGTH_PULL LLG_GMP_STRENGTH_PULL
#define LLG_STRENGTH_STRONG LLG_GMP_STRENGTH_STRONG
#define LLG_STRENGTH_SUPPLY LLG_GMP_STRENGTH_SUPPLY
/* V05/S7 public names. */
#define sv4_bitstoreal llg_gmp_sv4_bitstoreal
#define sv4_bitstoshortreal llg_gmp_sv4_bitstoshortreal
#define sv4_delay_ticks llg_gmp_sv4_delay_ticks
#define sv4_from_real llg_gmp_sv4_from_real
#define sv4_real_delay_ticks llg_gmp_sv4_real_delay_ticks
#define sv4_realtobits llg_gmp_sv4_realtobits
#define sv4_rtoi llg_gmp_sv4_rtoi
#define sv4_shortrealtobits llg_gmp_sv4_shortrealtobits
#define sv4_to_real llg_gmp_sv4_to_real
#define llg_real_to_bool llg_gmp_real_to_bool
/* V05/S8 public names. */
#define sv4_fits_i64 llg_gmp_sv4_fits_i64
#define sv4_format llg_gmp_sv4_format
#define sv4_to_dec_string llg_gmp_sv4_to_dec_string
#define sv4_to_i64 llg_gmp_sv4_to_i64
#define sv4_to_index llg_gmp_sv4_to_index
#define sv4_to_index_i64 llg_gmp_sv4_to_index_i64
/* V05/S9 public names. */
#define sv4_checked_width llg_gmp_sv4_checked_width
#define sv4_t llg_gmp_sv4_t
#define llg_sv4_word_t llg_gmp_sv4_word_t
#define llg_sv4_vpi_word_t llg_gmp_sv4_vpi_word_t
#define LLG_SV4_BITS 0
#define LLG_SV4_X 1
#define LLG_SV4_Z 2
#define LLG_VALUE_ABI_VERSION LLG_GMP_VALUE_ABI_VERSION
#define LLG_SUPPORTED_WIDTH_LIMIT LLG_GMP_SUPPORTED_WIDTH_LIMIT
#define SV4_LITERAL LLG_GMP_SV4_LITERAL
#define SV4_EMPTY LLG_GMP_SV4_EMPTY
#define LLG_MASK(w) LLG_GMP_MASK(w)
#define SV4_INIT(b, x, z, w, s) sv4_from_masks(b, x, z, w, s)
#define SV4_C(v, w) sv4_from_u64(v, w, 0)
#define SV4_S(v, w) sv4_from_u64(v, w, 1)
#define SV4_X(w) sv4_x(w, 0)
#define SV4_Z(w) sv4_fill(3, w, 0)
#endif
#endif
