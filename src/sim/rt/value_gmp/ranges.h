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
/* Reverse an in-bounds source interval into nonoverlapping destination bits.
 * Selected writes inspect only that interval before adding a B plane. */
static inline void g4_copy_reverse(g4_t* dst, uint32_t dst_bit, g4_t src, uint32_t src_bit,
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
    uint32_t high = src_bit + count;
    while (count) {
        uint32_t shift = dst_bit % 64u, take = 64u - shift;
        if (take > count)
            take = count;
        high -= take;
        uint64_t a = g4_reverse_word(g4_range_word(sa, high, take)) >> (64u - take);
        uint64_t b = sb ? g4_reverse_word(g4_range_word(sb, high, take)) >> (64u - take) : 0;
        uint64_t mask = g4_mask(take) << shift;
        uint32_t limb = dst_bit / 64u;
        da[limb] = (da[limb] & ~mask) | (a << shift);
        if (db)
            db[limb] = (db[limb] & ~mask) | (b << shift);
        dst_bit += take;
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
/* Decreasing source coordinates; clip before arithmetic on extreme indices. */
static inline void g4_reverse_window(g4_t* dst, int64_t dst_bit, g4_t src, int64_t src_high,
                                     uint32_t count) {
    if (!count || dst_bit >= (int64_t)dst->width || dst_bit <= -(int64_t)count)
        return;
    uint32_t skip = dst_bit < 0 ? (uint32_t)-dst_bit : 0;
    uint32_t start = (uint32_t)(dst_bit + skip), length = count - skip;
    if (length > dst->width - start)
        length = dst->width - start;
    if (src_high < 0 || src_high >= (int64_t)src.width + count) {
        g4_fill_bits(dst, start, length, 2);
        return;
    }
    src_high -= skip;
    if (src_high >= (int64_t)src.width) {
        uint32_t missing = (uint32_t)(src_high - src.width + 1);
        if (missing > length)
            missing = length;
        g4_fill_bits(dst, start, missing, 2);
        start += missing;
        length -= missing;
        src_high -= missing;
    }
    uint32_t available = src_high >= 0 ? (uint32_t)src_high + 1 : 0;
    uint32_t take = length < available ? length : available;
    if (take)
        g4_copy_reverse(dst, start, src, (uint32_t)(src_high + 1 - take), take);
    g4_fill_bits(dst, start + take, length - take, 2);
}
static inline int g4_alias(g4_t a, g4_t b) {
    return a.width > 64 && b.width > 64 && a.data.wide.a == b.data.wide.a;
}
#endif
