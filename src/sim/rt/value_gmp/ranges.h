#ifndef LLG_SV4_COMPACT_RANGES_H
#define LLG_SV4_COMPACT_RANGES_H
#include "internal.h"

static inline uint64_t g4_range_word(const uint64_t* plane, uint32_t bit, uint32_t count) {
    if (!plane)
        return 0;
    uint32_t shift = bit % 64u;
    uint64_t word = plane[bit / 64u] >> shift;
    if (count > 64u - shift)
        word |= plane[bit / 64u + 1u] << (64u - shift);
    return word & g4_mask(count);
}
/* In-bounds, nonoverlapping intervals. Finish only at the publication boundary,
 * so a multi-range operation cannot invalidate its borrowed payload mid-copy. */
static inline void g4_copy_bits(g4_t* dst, uint32_t dst_bit, g4_t src, uint32_t src_bit,
                                uint32_t count) {
    const uint64_t *sa = g4_a(&src), *sb = g4_b(&src);
    if (dst->width > 64 && !dst->data.wide.b && sb) {
        for (uint32_t bit = src_bit, remaining = count; remaining;) {
            uint32_t take = remaining < 64u ? remaining : 64u;
            if (g4_range_word(sb, bit, take)) {
                llg_gmp_sv4_promote(dst);
                break;
            }
            bit += take;
            remaining -= take;
        }
    }
    uint64_t *da = g4_mut_a(dst), *db = g4_mut_b(dst);
    while (count) {
        uint32_t shift = dst_bit % 64u, take = 64u - shift;
        if (take > count)
            take = count;
        uint64_t a = g4_range_word(sa, src_bit, take);
        uint64_t b = g4_range_word(sb, src_bit, take);
        uint64_t mask = g4_mask(take) << shift;
        uint32_t limb = dst_bit / 64u;
        da[limb] = (da[limb] & ~mask) | (a << shift);
        if (db)
            db[limb] = (db[limb] & ~mask) | (b << shift);
        dst_bit += take;
        src_bit += take;
        count -= take;
    }
}
static inline void g4_fill_bits(g4_t* dst, uint32_t bit, uint32_t count, int state) {
    if (count && state >= 2 && dst->width > 64 && !dst->data.wide.b)
        llg_gmp_sv4_promote(dst);
    uint64_t *a = g4_mut_a(dst), *b = g4_mut_b(dst);
    while (count) {
        uint32_t shift = bit % 64u, take = 64u - shift;
        if (take > count)
            take = count;
        uint64_t mask = g4_mask(take) << shift;
        uint32_t limb = bit / 64u;
        a[limb] = (a[limb] & ~mask) | (state == 1 || state == 2 ? mask : 0);
        if (b)
            b[limb] = (b[limb] & ~mask) | (state >= 2 ? mask : 0);
        bit += take;
        count -= take;
    }
}
static inline void g4_copy_window(g4_t* dst, int64_t dst_bit, g4_t src, int64_t src_bit,
                                  uint32_t count) {
    if (!count || dst_bit >= (int64_t)dst->width || dst_bit <= -(int64_t)count)
        return;
    uint32_t skip = dst_bit < 0 ? (uint32_t)-dst_bit : 0;
    uint32_t start = (uint32_t)(dst_bit + skip), length = count - skip;
    if (length > dst->width - start)
        length = dst->width - start;
    if (src_bit >= (int64_t)src.width || src_bit <= -(int64_t)count) {
        g4_fill_bits(dst, start, length, 2);
        return;
    }
    src_bit += skip;
    if (src_bit < 0) {
        uint32_t missing = (uint32_t)-src_bit;
        if (missing > length)
            missing = length;
        g4_fill_bits(dst, start, missing, 2);
        start += missing;
        length -= missing;
        src_bit += missing;
    }
    if (!length)
        return;
    uint32_t available = src_bit < (int64_t)src.width ? src.width - (uint32_t)src_bit : 0;
    uint32_t take = length < available ? length : available;
    g4_copy_bits(dst, start, src, (uint32_t)src_bit, take);
    g4_fill_bits(dst, start + take, length - take, 2);
}
static inline int g4_alias(g4_t a, g4_t b) {
    return a.width > 64 && b.width > 64 && a.data.wide.a == b.data.wide.a;
}
static inline int g4_range_state(g4_t v, int64_t bit) {
    if (bit < 0 || bit >= v.width)
        return 2;
    uint64_t a = g4_a(&v)[(uint32_t)bit / 64u];
    const uint64_t* b = g4_b(&v);
    uint32_t shift = (uint32_t)bit % 64u;
    unsigned known = (unsigned)((a >> shift) & 1u);
    return b && ((b[(uint32_t)bit / 64u] >> shift) & 1u) ? (known ? 2 : 3) : (int)known;
}
static inline void g4_put_state(g4_t* v, int64_t bit, int state) {
    if (bit < 0 || bit >= v->width)
        return;
    if (state >= 2 && v->width > 64 && !v->data.wide.b)
        llg_gmp_sv4_promote(v);
    uint32_t word = (uint32_t)bit / 64u;
    uint64_t mask = UINT64_C(1) << ((uint32_t)bit % 64u);
    uint64_t *a = g4_mut_a(v), *b = g4_mut_b(v);
    a[word] = (a[word] & ~mask) | (state == 1 || state == 2 ? mask : 0);
    if (b)
        b[word] = (b[word] & ~mask) | (state >= 2 ? mask : 0);
}
#endif
