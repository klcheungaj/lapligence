#include "ranges.h"

/* Capture aliases before promotion: realloc may move the mask's borrowed A
 * plane. No snapshots are needed because each mask word is read before writing. */
static void masked_write(g4_t* target, g4_t source, g4_t mask, int clipped) {
    if (g4_alias(*target, source))
        return;
    uint32_t width = target->width;
    if (clipped && width > source.width)
        width = source.width;
    size_t n = ((size_t)width + 63u) / 64u;
    size_t sn = llg_gmp_sv4_words(source), mn = llg_gmp_sv4_words(mask);
    if (n > sn)
        n = sn;
    if (n > mn)
        n = mn;
    int mask_alias = g4_alias(*target, mask);
    const uint64_t *sa = g4_a(&source), *sb = g4_b(&source);
    const uint64_t *ma = g4_a(&mask), *mb = g4_b(&mask);
    if (target->width > 64 && !target->data.wide.b && sb) {
        for (size_t i = 0; i < n; ++i) {
            uint64_t selected = ma[i] & ~(mb ? mb[i] : 0);
            if (clipped && i + 1 == n && n == ((size_t)width + 63u) / 64u)
                selected &= g4_topmask(width);
            if (selected & sb[i]) {
                llg_gmp_sv4_promote(target);
                if (mask_alias) {
                    ma = g4_a(target);
                    mb = g4_b(target);
                }
                break;
            }
        }
    }
    uint64_t *a = g4_mut_a(target), *b = g4_mut_b(target);
    for (size_t i = 0; i < n; ++i) {
        uint64_t selected = ma[i] & ~(mb ? mb[i] : 0);
        if (clipped && i + 1 == n && n == ((size_t)width + 63u) / 64u)
            selected &= g4_topmask(width);
        a[i] = (a[i] & ~selected) | (sa[i] & selected);
        if (b)
            b[i] = (b[i] & ~selected) | ((sb ? sb[i] : 0) & selected);
    }
    llg_gmp_sv4_finish(target);
}
void llg_gmp_sv4_masked_merge(g4_t* target, g4_t source, g4_t mask) {
    masked_write(target, source, mask, 0);
}
void llg_gmp_sv4_masked_copy(g4_t* target, g4_t source, g4_t mask) {
    masked_write(target, source, mask, 1);
}
int llg_gmp_sv4_masked_same(g4_t a, g4_t b, const g4_t* mask) {
    uint32_t width = a.width < b.width ? a.width : b.width;
    size_t n = ((size_t)width + 63u) / 64u;
    const uint64_t *aa = g4_a(&a), *ab = g4_b(&a), *ba = g4_a(&b), *bb = g4_b(&b);
    size_t mn = mask ? llg_gmp_sv4_words(*mask) : 0;
    const uint64_t *ma = mask ? g4_a(mask) : NULL, *mb = mask ? g4_b(mask) : NULL;
    for (size_t i = 0; i < n; ++i) {
        uint64_t selected = mask ? (i < mn ? ma[i] & ~(mb ? mb[i] : 0) : 0) : UINT64_MAX;
        if (i + 1 == n)
            selected &= g4_topmask(width);
        if (((aa[i] ^ ba[i]) | ((ab ? ab[i] : 0) ^ (bb ? bb[i] : 0))) & selected)
            return 0;
    }
    return 1;
}
int llg_gmp_sv4_range_same(g4_t target, uint32_t offset, g4_t source) {
    const uint64_t *ta = g4_a(&target), *tb = g4_b(&target);
    const uint64_t *sa = g4_a(&source), *sb = g4_b(&source);
    for (uint32_t bit = 0; bit < source.width;) {
        unsigned take = source.width - bit < 64 ? source.width - bit : 64;
        if (g4_range_word(ta, offset + bit, take) != sa[bit / 64] ||
            g4_range_word(tb, offset + bit, take) != (sb ? sb[bit / 64] : 0))
            return 0;
        bit += take;
    }
    return 1;
}
void llg_gmp_sv4_range_copy(g4_t* target, uint32_t offset, g4_t source) {
    if (!source.width || g4_alias(*target, source))
        return;
    g4_copy_bits(target, offset, source, 0, source.width);
    llg_gmp_sv4_finish(target);
}
void llg_gmp_sv4_range_fill(g4_t* target, uint32_t low, uint32_t count, unsigned state) {
    if (low >= target->width || !count)
        return;
    if (count > target->width - low)
        count = target->width - low;
    g4_fill_bits(target, low, count, state <= 3 ? (int)state : 0);
    llg_gmp_sv4_finish(target);
}
void llg_gmp_sv4_mask_remove(g4_t* target, g4_t removed) {
    uint64_t* a = g4_mut_a(target);
    const uint64_t *b = g4_b(target), *ra = g4_a(&removed), *rb = g4_b(&removed);
    size_t n = llg_gmp_sv4_words(*target), rn = llg_gmp_sv4_words(removed);
    if (n > rn)
        n = rn;
    for (size_t i = 0; i < n; ++i)
        a[i] &= ~(ra[i] & ~(rb ? rb[i] : 0) & ~(b ? b[i] : 0));
}
void llg_gmp_sv4_mask_top_wide(g4_t* value) { llg_gmp_sv4_finish(value); }
void llg_gmp_sv4_mul_add_known_wide(g4_t* value, uint32_t factor, uint32_t addend) {
    uint64_t* a = g4_mut_a(value);
    uint64_t carry = addend;
    for (size_t i = 0; i < llg_gmp_sv4_words(*value); ++i) {
        uint64_t low = (a[i] & UINT32_MAX) * factor + carry;
        uint64_t high = (a[i] >> 32) * factor + (low >> 32);
        a[i] = (high << 32) | (low & UINT32_MAX);
        carry = high >> 32;
    }
    llg_gmp_sv4_finish(value);
}
void llg_gmp_sv4_negate_known_wide(g4_t* value) {
    uint64_t* a = g4_mut_a(value);
    uint64_t carry = 1;
    for (size_t i = 0; i < llg_gmp_sv4_words(*value); ++i) {
        uint64_t inverted = ~a[i];
        a[i] = inverted + carry;
        carry = a[i] < inverted;
    }
    llg_gmp_sv4_finish(value);
}
void llg_gmp_sv4_two_state_inplace_wide(g4_t* value) {
    uint64_t *a = g4_mut_a(value), *b = g4_mut_b(value);
    if (!b)
        return;
    for (size_t i = 0; i < llg_gmp_sv4_words(*value); ++i) {
        a[i] &= ~b[i];
        b[i] = 0;
    }
    llg_gmp_sv4_finish(value);
}
void llg_gmp_sv4_append_digit_wide(g4_t* value, unsigned count, unsigned state, unsigned digit) {
    if (!value->width)
        return;
    uint64_t low_mask = g4_mask(count);
    uint64_t carry_a = state == 0 ? digit & low_mask : state == 2 ? low_mask : 0;
    uint64_t carry_b = state == 2 || state == 3 ? low_mask : 0;
    if (carry_b)
        llg_gmp_sv4_promote(value);
    uint64_t *a = g4_mut_a(value), *b = g4_mut_b(value);
    for (size_t i = 0; i < llg_gmp_sv4_words(*value); ++i) {
        uint64_t old_a = a[i], old_b = b ? b[i] : 0;
        a[i] = (old_a << count) | carry_a;
        if (b)
            b[i] = (old_b << count) | carry_b;
        carry_a = old_a >> (64u - count);
        carry_b = old_b >> (64u - count);
    }
    llg_gmp_sv4_finish(value);
}
void llg_gmp_sv4_export_vpi32(g4_t value, void* output, size_t count, size_t stride) {
    const uint64_t *a = g4_a(&value), *b = g4_b(&value);
    unsigned char* bytes = (unsigned char*)output;
    size_t n = llg_gmp_sv4_words(value);
    for (size_t i = 0; i < count; ++i) {
        unsigned shift = (unsigned)(i % 2) * 32;
        uint32_t aval = i / 2 < n ? (uint32_t)(a[i / 2] >> shift) : 0;
        uint32_t bval = i / 2 < n && b ? (uint32_t)(b[i / 2] >> shift) : 0;
        memcpy(bytes + i * stride, &aval, sizeof(aval));
        memcpy(bytes + i * stride + 4, &bval, sizeof(bval));
    }
}
void llg_gmp_sv4_import_vpi32(g4_t* value, const void* input, size_t count, size_t stride) {
    size_t available = ((size_t)value->width + 31u) / 32u;
    if (count > available)
        count = available;
    if (!count)
        return;
    const unsigned char* bytes = (const unsigned char*)input;
    if (value->width > 64 && !value->data.wide.b) {
        for (size_t i = 0; i < count; ++i) {
            uint32_t bval;
            memcpy(&bval, bytes + i * stride + 4, sizeof(bval));
            uint32_t remaining = value->width - (uint32_t)(i * 32u);
            if (bval & g4_mask(remaining < 32 ? remaining : 32)) {
                llg_gmp_sv4_promote(value);
                break;
            }
        }
    }
    uint64_t *a = g4_mut_a(value), *b = g4_mut_b(value);
    for (size_t i = 0; i < count; ++i) {
        uint32_t aval, bval;
        memcpy(&aval, bytes + i * stride, sizeof(aval));
        memcpy(&bval, bytes + i * stride + 4, sizeof(bval));
        unsigned shift = (unsigned)(i % 2) * 32;
        uint64_t mask = (uint64_t)UINT32_MAX << shift;
        a[i / 2] = (a[i / 2] & ~mask) | ((uint64_t)aval << shift);
        if (b)
            b[i / 2] = (b[i / 2] & ~mask) | ((uint64_t)bval << shift);
    }
    llg_gmp_sv4_finish(value);
}
int llg_gmp_sv4_same_vpi_words(g4_t value, const llg_gmp_sv4_vpi_word_t* words, size_t count) {
    size_t n = llg_gmp_sv4_words(value), scan = count > n ? count : n;
    const uint64_t *a = g4_a(&value), *b = g4_b(&value);
    for (size_t i = 0; i < scan; ++i) {
        uint64_t wa = i < count ? words[i].aval : 0, wb = i < count ? words[i].bval : 0;
        if (wa != (i < n ? a[i] : 0) || wb != (i < n && b ? b[i] : 0))
            return 0;
    }
    return 1;
}
void llg_gmp_sv4_export_text(g4_t value, uint32_t width, char* output) {
    const uint64_t *a = g4_a(&value), *b = g4_b(&value);
    size_t n = llg_gmp_sv4_words(value);
    for (size_t i = 0; i < ((size_t)width + 63u) / 64u; ++i) {
        uint64_t wa = i < n ? a[i] : 0, wb = i < n && b ? b[i] : 0;
        unsigned take = width - (uint32_t)(i * 64u);
        if (take > 64)
            take = 64;
        for (unsigned bit = 0; bit < take; ++bit) {
            output[width - 1u - (uint32_t)(i * 64u) - bit] =
                "01zx"[((wa >> bit) & 1u) | (((wb >> bit) & 1u) << 1)];
        }
    }
    output[width] = 0;
}
