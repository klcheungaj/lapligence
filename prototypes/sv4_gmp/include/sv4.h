#ifndef LLG_PROTOTYPE_SV4_H
#define LLG_PROTOTYPE_SV4_H
#ifndef LLG_SV4_USE_GMP
#define LLG_SV4_USE_GMP 0
#endif
#define LLG_SV4_SELECTOR_0_TOKEN 1
#define LLG_SV4_SELECTOR_1_TOKEN 1
#define LLG_SV4_SELECTOR_CHECK_I(value) LLG_SV4_SELECTOR_##value##_TOKEN
#define LLG_SV4_SELECTOR_CHECK(value) LLG_SV4_SELECTOR_CHECK_I(value)
#if !LLG_SV4_SELECTOR_CHECK(LLG_SV4_USE_GMP)
#error "LLG_SV4_USE_GMP must be 0 or 1"
#endif
#undef LLG_SV4_SELECTOR_CHECK
#undef LLG_SV4_SELECTOR_CHECK_I
#undef LLG_SV4_SELECTOR_0_TOKEN
#undef LLG_SV4_SELECTOR_1_TOKEN
#if LLG_SV4_USE_GMP
#include "gmp4.h"
typedef gmp4_t sv4_t;
#define SV4_EMPTY GMP4_EMPTY
#define LLG_VALUE_ABI_VERSION GMP4_PROTOTYPE_ABI
#define LLG_SUPPORTED_WIDTH_LIMIT (1u << 20)
#define LLG_MASK(w) ((w) >= 64 ? ~0ULL : ((1ULL << (w)) - 1))
#define SV4_INIT(b, x, z, w, s) \
    sv4_from_masks((uint64_t)(b), (uint64_t)(x), (uint64_t)(z), (w), (s))
#define SV4_C(b, w) sv4_from_u64((uint64_t)(b), (w), 0)
#define SV4_S(b, w) sv4_from_u64((uint64_t)(b), (w), 1)
#define SV4_X(w) sv4_x((w), 0)
#define SV4_Z(w) sv4_fill(3, (w), 0)
#define LLG_SV4_BACKEND_NAME "gmp-prototype"
#define sv4_from_u64 gmp4_from_u64
#define sv4_from_i64 gmp4_from_i64
#define sv4_to_u64 gmp4_to_u64
#define sv4_zero gmp4_zero
#define sv4_fill gmp4_fill
#define sv4_x gmp4_x
#define sv4_from_limbs gmp4_from_limbs
#define sv4_from_masks gmp4_from_masks
#define sv4_clone gmp4_clone
#define sv4_copy gmp4_copy
#define sv4_assign gmp4_assign
#define sv4_move gmp4_move
#define sv4_replace gmp4_replace
#define sv4_destroy gmp4_destroy
#define sv4_destroy_array gmp4_destroy_array
#define sv4_bytes gmp4_bytes
#define sv4_is_unknown gmp4_is_unknown
#define sv4_to_bool gmp4_to_bool
#define sv4_same gmp4_same
#define sv4_resize gmp4_resize
#define sv4_cast gmp4_cast
#define sv4_to_two_state gmp4_to_two_state
#define sv4_add gmp4_add
#define sv4_sub gmp4_sub
#define sv4_mul gmp4_mul
#define sv4_neg gmp4_neg
#define sv4_bitneg gmp4_bitneg
#define sv4_and gmp4_and
#define sv4_or gmp4_or
#define sv4_xor gmp4_xor
#define sv4_xnor gmp4_xnor
#define sv4_eq gmp4_eq
#define sv4_case_eq gmp4_case_eq
#define sv4_lognot gmp4_lognot
#define sv4_logand gmp4_logand
#define sv4_logor gmp4_logor
#define sv4_mux gmp4_mux
#else
#include "../golden/llg_value.h"
#define LLG_SV4_BACKEND_NAME "legacy-golden"
#endif

/* Representation-neutral bridge; see src/sim/rt/value/facade.md. Buffers are
 * caller-owned copies, never views into payload storage. */
typedef struct { uint64_t bits, x, z; } llg_sv4_word_t;
typedef struct { uint64_t aval, bval; } llg_sv4_vpi_word_t;
enum { LLG_SV4_BITS = 0, LLG_SV4_X = 1, LLG_SV4_Z = 2 };

static inline uint32_t llg_sv4_width(sv4_t value) { return value.width; }
static inline int8_t llg_sv4_signed(sv4_t value) { return value.is_signed; }
static inline void llg_sv4_set_signed(sv4_t* value, int8_t sign) {
    value->is_signed = (int8_t)(sign != 0);
}
static inline size_t llg_sv4_words(sv4_t value) {
    return ((size_t)value.width + 63u) / 64u;
}
static inline uint64_t llg_sv4_word(sv4_t value, size_t word, unsigned plane) {
#if LLG_SV4_USE_GMP
    return gmp4_word(value, word, plane);
#else
    if (word >= llg_sv4_words(value)) return 0;
    return plane == LLG_SV4_BITS ? value.bits[word] :
           plane == LLG_SV4_X ? value.x[word] : value.z[word];
#endif
}
static inline unsigned llg_sv4_state(sv4_t value, uint64_t bit) {
#if LLG_SV4_USE_GMP
    return gmp4_get_bit(value, bit);
#else
    if (bit >= value.width) return 2;
    size_t word = (size_t)(bit / 64u);
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    return value.x[word] & mask ? 2u : value.z[word] & mask ? 3u :
           (value.bits[word] & mask) != 0;
#endif
}
static inline void llg_sv4_set_state(sv4_t* value, uint64_t bit, unsigned state) {
#if LLG_SV4_USE_GMP
    gmp4_set_bit(value, bit, state);
#else
    if (bit >= value->width) return;
    size_t word = (size_t)(bit / 64u);
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    value->bits[word] = (value->bits[word] & ~mask) | (state == 1 ? mask : 0);
    value->x[word] = (value->x[word] & ~mask) | (state == 2 ? mask : 0);
    value->z[word] = (value->z[word] & ~mask) | (state == 3 ? mask : 0);
#endif
}
static inline void llg_sv4_set_word(sv4_t* value, size_t word,
                                    uint64_t bits, uint64_t x, uint64_t z) {
#if LLG_SV4_USE_GMP
    gmp4_set_word(value, word, bits, x, z);
#else
    if (word >= llg_sv4_words(*value)) return;
    uint32_t remaining = value->width - (uint32_t)(word * 64u);
    uint64_t mask = remaining >= 64u ? UINT64_MAX :
                    UINT64_MAX >> (64u - remaining);
    x &= mask;
    z &= mask & ~x;
    value->bits[word] = bits & mask & ~(x | z);
    value->x[word] = x;
    value->z[word] = z;
#endif
}
static inline llg_sv4_vpi_word_t llg_sv4_vpi_word(sv4_t value, size_t word) {
    uint64_t bits = llg_sv4_word(value, word, LLG_SV4_BITS);
    uint64_t x = llg_sv4_word(value, word, LLG_SV4_X);
    uint64_t z = llg_sv4_word(value, word, LLG_SV4_Z);
    llg_sv4_vpi_word_t result = { bits | x, x | z };
    return result;
}
static inline void llg_sv4_set_vpi_word(sv4_t* value, size_t word,
                                        llg_sv4_vpi_word_t input) {
    llg_sv4_set_word(value, word, input.aval & ~input.bval,
                    input.aval & input.bval, ~input.aval & input.bval);
}
static inline int llg_sv4_has_x(sv4_t value) {
    for (size_t i = 0; i < llg_sv4_words(value); ++i)
        if (llg_sv4_word(value, i, LLG_SV4_X)) return 1;
    return 0;
}
static inline int llg_sv4_has_z(sv4_t value) {
    for (size_t i = 0; i < llg_sv4_words(value); ++i)
        if (llg_sv4_word(value, i, LLG_SV4_Z)) return 1;
    return 0;
}
static inline unsigned llg_sv4_state_to_dpi(unsigned state) {
    return state < 2u ? state : state ^ 1u;
}
static inline unsigned llg_sv4_state_from_dpi(unsigned state) {
    return state < 2u ? state : state ^ 1u;
}
static inline size_t llg_sv4_word_range(sv4_t value, size_t first, size_t count) {
    size_t words = llg_sv4_words(value);
    if (first >= words) return 0;
    size_t available = words - first;
    return count < available ? count : available;
}
static inline void llg_sv4_export_words(sv4_t value, size_t first,
                                        llg_sv4_word_t* output, size_t count) {
    size_t available = llg_sv4_word_range(value, first, count);
    for (size_t i = 0; i < count; ++i) {
        llg_sv4_word_t word = {0, 0, 0};
        if (i < available) {
            word.bits = llg_sv4_word(value, first + i, LLG_SV4_BITS);
            word.x = llg_sv4_word(value, first + i, LLG_SV4_X);
            word.z = llg_sv4_word(value, first + i, LLG_SV4_Z);
        }
        output[i] = word;
    }
}
static inline void llg_sv4_import_words(sv4_t* value, size_t first,
                                        const llg_sv4_word_t* input, size_t count) {
    size_t available = llg_sv4_word_range(*value, first, count);
    for (size_t i = 0; i < available; ++i)
        llg_sv4_set_word(value, first + i, input[i].bits, input[i].x, input[i].z);
}
static inline void llg_sv4_export_vpi_words(sv4_t value, size_t first,
                                            llg_sv4_vpi_word_t* output, size_t count) {
    size_t available = llg_sv4_word_range(value, first, count);
    for (size_t i = 0; i < count; ++i) {
        llg_sv4_vpi_word_t word = {0, 0};
        if (i < available) word = llg_sv4_vpi_word(value, first + i);
        output[i] = word;
    }
}
static inline void llg_sv4_import_vpi_words(sv4_t* value, size_t first,
                                            const llg_sv4_vpi_word_t* input, size_t count) {
    size_t available = llg_sv4_word_range(*value, first, count);
    for (size_t i = 0; i < available; ++i)
        llg_sv4_set_vpi_word(value, first + i, input[i]);
}

#endif
