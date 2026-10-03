#ifndef LLG_SV4_COMPACT_BACKEND_H
#define LLG_SV4_COMPACT_BACKEND_H

#include <stddef.h>
#include <stdint.h>

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

#ifdef __cplusplus
}
#endif

/* Standalone facade. V07 owns selection through llg_value.h. */
#ifdef LLG_SV4_GMP_PUBLIC_NAMES
#define sv4_add llg_gmp_sv4_add
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
