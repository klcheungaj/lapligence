#include "ranges.h"

/* SV2009 11.4.11 compares immediate array elements with logical equality.
 * A matching X/Z payload therefore still selects the element default. */
g4_t llg_gmp_sv4_array_conditional_merge_wide(g4_t a, g4_t b, g4_t element_default) {
    uint32_t stride = element_default.width;
    if (!a.width || a.width >= LLG_GMP_SUPPORTED_WIDTH_LIMIT || a.width != b.width || !stride ||
        stride > a.width || a.width % stride)
        llg_gmp_sv4_fail("invalid array conditional merge shape");
    g4_t result = llg_gmp_sv4_zero(a.width, 0);
    const uint64_t *aa = g4_a(&a), *ab = g4_b(&a), *ba = g4_a(&b), *bb = g4_b(&b);
    if (stride == 1) {
        uint64_t default_a = element_default.data.small.a ? UINT64_MAX : 0;
        uint64_t default_b = element_default.data.small.b ? UINT64_MAX : 0;
        size_t n = llg_gmp_sv4_words(a);
        for (size_t i = 0; i < n; ++i) {
            uint64_t mask = i + 1 == n ? g4_topmask(a.width) : UINT64_MAX;
            uint64_t different = ((aa[i] ^ ba[i]) | (ab ? ab[i] : 0) | (bb ? bb[i] : 0)) & mask;
            uint64_t unknown = default_b & different;
            if (unknown && result.width > 64 && !result.data.wide.b)
                llg_gmp_sv4_promote(&result);
            g4_mut_a(&result)[i] = (aa[i] & ~different) | (default_a & different);
            uint64_t* out_b = g4_mut_b(&result);
            if (out_b)
                out_b[i] = unknown;
        }
        llg_gmp_sv4_finish(&result);
        return result;
    }
    for (uint32_t offset = 0; offset < a.width; offset += stride) {
        int equal = 1;
        for (uint32_t bit = 0; bit < stride;) {
            uint32_t take = stride - bit;
            if (take > 64u)
                take = 64u;
            uint32_t start = offset + bit;
            if (g4_range_word(ab, start, take) || g4_range_word(bb, start, take) ||
                g4_range_word(aa, start, take) != g4_range_word(ba, start, take)) {
                equal = 0;
                break;
            }
            bit += take;
        }
        g4_copy_bits(&result, offset, equal ? a : element_default, equal ? offset : 0, stride);
    }
    llg_gmp_sv4_finish(&result);
    return result;
}
/* Full assemblies preserve all input bits. B presence is known before
 * allocation, and bounded copies leave zero padding without a final scan. */
g4_t llg_gmp_sv4_concat_wide(g4_t hi, g4_t lo) {
    uint32_t width = g4_selection_width((uint64_t)hi.width + lo.width);
    g4_t result =
        llg_gmp_sv4_new(width, 0, llg_gmp_sv4_is_unknown(hi) || llg_gmp_sv4_is_unknown(lo));
    if (width > 64 && hi.width <= 64 && lo.width <= 64) {
        uint64_t* a = g4_mut_a(&result);
        uint64_t* b = g4_mut_b(&result);
        a[0] = lo.data.small.a | (lo.width < 64 ? hi.data.small.a << lo.width : 0);
        a[1] = lo.width == 64 ? hi.data.small.a : hi.data.small.a >> (64 - lo.width);
        if (b) {
            b[0] = lo.data.small.b | (lo.width < 64 ? hi.data.small.b << lo.width : 0);
            b[1] = lo.width == 64 ? hi.data.small.b : hi.data.small.b >> (64 - lo.width);
        }
        return result;
    }
    g4_copy_bits(&result, 0, lo, 0, lo.width);
    g4_copy_bits(&result, lo.width, hi, 0, hi.width);
    return result;
}
g4_t llg_gmp_sv4_repeat_wide(g4_t pattern, uint64_t count) {
    if (pattern.width && count > UINT64_MAX / pattern.width)
        llg_gmp_sv4_fail("width reaches exclusive limit");
    uint32_t width = g4_selection_width((uint64_t)pattern.width * count);
    g4_t result = llg_gmp_sv4_new(width, 0, width && llg_gmp_sv4_is_unknown(pattern));
    if (width)
        g4_copy_bits(&result, 0, pattern, 0, pattern.width);
    for (uint32_t copied = pattern.width; copied < width;) {
        uint32_t take = width - copied;
        if (take > copied)
            take = copied;
        /* The initialized prefix and next block are disjoint. Its B plane,
         * if needed, was allocated before the initial pattern copy. */
        g4_copy_bits(&result, copied, result, 0, take);
        copied += take;
    }
    return result;
}
g4_t llg_gmp_sv4_repeat_count_wide(g4_t value) {
    g4_width_check(value.width);
    if (llg_gmp_sv4_is_unknown(value) ||
        (value.is_signed && value.width && llg_gmp_sv4_state(value, value.width - 1) == 1))
        return llg_gmp_sv4_zero(value.width, 0);
    value.is_signed = 0;
    return llg_gmp_sv4_clone(&value);
}
static g4_t g4_stream(g4_t value, uint32_t slice, int reverse, int inverse) {
    if (!slice)
        llg_gmp_sv4_fail("zero streaming slice size");
    if (!reverse || !value.width || slice >= value.width) {
        value.is_signed = 0;
        return llg_gmp_sv4_clone(&value);
    }
    g4_t result = llg_gmp_sv4_new(value.width, 0, llg_gmp_sv4_is_unknown(value));
    for (uint32_t bit = 0; bit < value.width;) {
        uint32_t take = value.width - bit;
        if (take > slice)
            take = slice;
        uint32_t other = value.width - bit - take;
        g4_copy_bits(&result, inverse ? bit : other, value, inverse ? other : bit, take);
        bit += take;
    }
    return result;
}
g4_t llg_gmp_sv4_stream_wide(g4_t value, uint32_t slice, int right_to_left) {
    return g4_stream(value, slice, right_to_left, 0);
}
g4_t llg_gmp_sv4_unstream_wide(g4_t value, uint32_t slice, int right_to_left) {
    return g4_stream(value, slice, right_to_left, 1);
}
