#ifndef LLG_PROTOTYPE_SV4_H
#define LLG_PROTOTYPE_SV4_H
#ifndef LLG_SV4_USE_GMP
#define LLG_SV4_USE_GMP 0
#endif
#if LLG_SV4_USE_GMP != 0 && LLG_SV4_USE_GMP != 1
#error "LLG_SV4_USE_GMP must be 0 or 1"
#endif
#if LLG_SV4_USE_GMP
#include "gmp4.h"
typedef gmp4_t sv4_t;
#define SV4_EMPTY GMP4_EMPTY
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

/* Backend selection adds no calls or branches. These migration inspectors
 * convert representation only; callers must not access private planes. */
static inline uint64_t llg_sv4_word(sv4_t value, size_t word, unsigned plane) {
#if LLG_SV4_USE_GMP
    return gmp4_word(value, word, plane);
#else
    if (word >= ((size_t)value.width + 63u) / 64u) return 0;
    return plane == 0 ? value.bits[word] : plane == 1 ? value.x[word] : value.z[word];
#endif
}
static inline unsigned llg_sv4_state(sv4_t value, uint64_t bit) {
    if (bit >= value.width) return 2;
#if LLG_SV4_USE_GMP
    return gmp4_get_bit(value, bit);
#else
    size_t word = (size_t)(bit / 64u);
    uint64_t mask = UINT64_C(1) << (bit % 64u);
    return value.x[word] & mask ? 2u : value.z[word] & mask ? 3u :
        (value.bits[word] & mask) != 0;
#endif
}
#endif
