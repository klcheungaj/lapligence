#ifndef LLG_PROTOTYPE_GMP4_INTERNAL_H
#define LLG_PROTOTYPE_GMP4_INTERNAL_H
#include "gmp4.h"
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef GMP4_ALLOC_HEADER
#include GMP4_ALLOC_HEADER
#endif
#ifndef GMP4_ALLOC
#define GMP4_ALLOC malloc
#endif
#ifndef GMP4_FREE
#define GMP4_FREE free
#endif

_Static_assert(GMP_NAIL_BITS == 0, "prototype requires nail-free GMP");
_Static_assert(GMP_NUMB_BITS == 32 || GMP_NUMB_BITS == 64,
               "prototype requires 32 or 64 bit GMP limbs");
_Static_assert(sizeof(mp_limb_t) * CHAR_BIT == GMP_NUMB_BITS,
               "GMP header/limb representation mismatch");

static inline void g4_fail(const char *message) {
    fprintf(stderr, "sv4 GMP prototype fatal: %s\n", message);
    abort();
}
static inline void g4_require_width(uint32_t width) {
    if (width >= GMP4_WIDTH_LIMIT) g4_fail("width reaches exclusive limit");
}
static inline size_t g4_count(uint32_t width) {
    return ((size_t)width + GMP_NUMB_BITS - 1u) / GMP_NUMB_BITS;
}
static inline uint64_t g4_mask64(unsigned bits) {
    return bits == 0 ? 0 : bits >= 64 ? UINT64_MAX : UINT64_MAX >> (64 - bits);
}
static inline mp_limb_t g4_mask(uint32_t width, size_t limb) {
    size_t first = limb * GMP_NUMB_BITS;
    if (first >= width) return 0;
    size_t remain = width - first;
    return remain >= GMP_NUMB_BITS ? GMP_NUMB_MASK :
           (mp_limb_t)(GMP_NUMB_MASK >> (GMP_NUMB_BITS - remain));
}
static inline mp_limb_t *g4_alloc(size_t limbs) {
    if (!limbs || limbs > SIZE_MAX / sizeof(mp_limb_t))
        g4_fail("invalid allocation size");
    mp_limb_t *result = (mp_limb_t *)GMP4_ALLOC(limbs * sizeof(mp_limb_t));
    if (!result) g4_fail("allocation failed");
    return result;
}
static inline int g4_has_b(gmp4_t value) {
    return value.width <= 64 ? value.data.small.b != 0 : value.data.wide.b != NULL;
}
static inline mp_limb_t g4_limb(gmp4_t value, size_t limb, unsigned plane) {
    if (limb >= g4_count(value.width)) return 0;
    if (value.width <= 64) {
        uint64_t word = plane ? value.data.small.b : value.data.small.a;
        return (mp_limb_t)(word >> (limb * GMP_NUMB_BITS));
    }
    const mp_limb_t *p = plane ? value.data.wide.b : value.data.wide.a;
    return p ? p[limb] : 0;
}
static inline void g4_put(gmp4_t *value, size_t limb, unsigned plane, mp_limb_t word) {
    word &= g4_mask(value->width, limb);
    if (value->width <= 64) {
        uint64_t *p = plane ? &value->data.small.b : &value->data.small.a;
        unsigned shift = (unsigned)(limb * GMP_NUMB_BITS);
        uint64_t mask = (uint64_t)GMP_NUMB_MASK << shift;
        *p = (*p & ~mask) | ((uint64_t)word << shift);
    } else {
        mp_limb_t *p = plane ? value->data.wide.b : value->data.wide.a;
        if (p) p[limb] = word;
        else if (word) g4_fail("missing unknown storage");
    }
}
/* Signed extension of either A/B plane, without constructing an operand. */
static inline mp_limb_t g4_extended(gmp4_t value, uint32_t width,
                                   int extend_signed, size_t limb, unsigned plane) {
    mp_limb_t source_mask = g4_mask(value.width, limb);
    mp_limb_t word = g4_limb(value, limb, plane) & source_mask;
    if (extend_signed && value.width && width > value.width) {
        size_t top = (value.width - 1u) / GMP_NUMB_BITS;
        unsigned shift = (value.width - 1u) % GMP_NUMB_BITS;
        if ((g4_limb(value, top, plane) >> shift) & 1u) word |= ~source_mask;
    }
    return word & g4_mask(width, limb);
}
static inline uint32_t g4_max_width(gmp4_t a, gmp4_t b) {
    return a.width > b.width ? a.width : b.width;
}
static inline uint64_t g4_small_extended(gmp4_t value, uint32_t width, int sign) {
    uint64_t a = value.data.small.a;
    if (sign && value.width && width > value.width &&
        ((a >> (value.width - 1u)) & 1u)) a |= ~g4_mask64(value.width);
    return a & g4_mask64(width);
}
gmp4_t g4_new(uint32_t width, int8_t is_signed, int with_b);
void g4_promote(gmp4_t *value);
int g4_truth(gmp4_t value);
void g4_workspace_reserve(gmp4_workspace_t *workspace, size_t limbs);
#endif
