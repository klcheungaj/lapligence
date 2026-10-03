#ifndef LLG_SV4_COMPACT_INTERNAL_H
#define LLG_SV4_COMPACT_INTERNAL_H
#include "backend.h"
#include <stdlib.h>
#include <string.h>

#ifndef LLG_SV4_GMP_KERNELS
#define LLG_SV4_GMP_KERNELS 0
#endif
static inline size_t g4_product_words(size_t n) {
#if LLG_SV4_GMP_KERNELS
    return n >= LLG_SV4_MUL_FULL_THRESHOLD ? 2u * n : n;
#else
    return n;
#endif
}

/* These pointer views live only for a synchronous operation. */
static inline const uint64_t* g4_a(const g4_t* v) {
    return v->width <= 64 ? &v->data.small.a : v->data.wide.a;
}
static inline const uint64_t* g4_b(const g4_t* v) {
    return v->width <= 64 ? &v->data.small.b : v->data.wide.b;
}
static inline uint64_t* g4_mut_a(g4_t* v) {
    return v->width <= 64 ? &v->data.small.a : v->data.wide.a;
}
static inline uint64_t* g4_mut_b(g4_t* v) {
    return v->width <= 64 ? &v->data.small.b : v->data.wide.b;
}
static inline uint64_t g4_topmask(uint32_t w) { return g4_mask(w % 64 ? w % 64 : 64); }
g4_t llg_gmp_sv4_new(uint32_t width, int8_t sign, int with_b);
uint64_t* llg_gmp_sv4_alloc(size_t words);
void llg_gmp_sv4_promote(g4_t* v);
void llg_gmp_sv4_finish(g4_t* v);
void llg_gmp_sv4_extend_plane(uint64_t* out, uint32_t width, const uint64_t* input, uint32_t source,
                              int sign);
void llg_gmp_sv4_extend_value(g4_t* out, g4_t input, int sign);
/* No aliases. Caller provides 2n result words only above the full threshold. */
void llg_gmp_sv4_kernel_mul(uint64_t* out, const uint64_t* a, const uint64_t* b, size_t n);
void llg_gmp_sv4_kernel_div(uint64_t* out, const uint64_t* a, const uint64_t* b, size_t n,
                            int remainder);
/* The caller supplies ceil(width*log10(2))+2 digit bytes and nonzero magnitude. */
size_t llg_gmp_sv4_kernel_decimal(unsigned char* out, uint64_t* magnitude, size_t n);
typedef struct {
    const uint64_t *a, *b;
    size_t full;
    uint64_t tail_a, tail_b, fill_a, fill_b;
} g4_span;
static inline g4_span g4_span_of(const g4_t* v, int sign) {
    g4_span s = {g4_a(v), g4_b(v), v->width / 64u, 0, 0, 0, 0};
    size_t n = llg_gmp_sv4_words(*v);
    if (sign && n) {
        unsigned shift = (v->width - 1u) % 64u;
        s.fill_a = ((s.a[n - 1] >> shift) & 1) ? UINT64_MAX : 0;
        s.fill_b = s.b && ((s.b[n - 1] >> shift) & 1) ? UINT64_MAX : 0;
    }
    if (v->width % 64u) {
        uint64_t pad = ~g4_topmask(v->width);
        s.tail_a = s.a[s.full] | (s.fill_a & pad);
        s.tail_b = (s.b ? s.b[s.full] : 0) | (s.fill_b & pad);
    } else {
        s.tail_a = s.fill_a;
        s.tail_b = s.fill_b;
    }
    return s;
}
/* Representation and extension are resolved once. The loops load pointers
 * directly; there is no per-limb function or public adapter call. */
#define G4_SPAN_A(s, i) ((i) < (s).full ? (s).a[i] : (i) == (s).full ? (s).tail_a : (s).fill_a)
#define G4_SPAN_B(s, i)                                                                            \
    ((i) < (s).full ? ((s).b ? (s).b[i] : 0) : (i) == (s).full ? (s).tail_b : (s).fill_b)
#endif
