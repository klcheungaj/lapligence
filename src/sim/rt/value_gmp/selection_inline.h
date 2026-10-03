#ifndef LLG_SV4_COMPACT_SELECTION_INLINE_H
#define LLG_SV4_COMPACT_SELECTION_INLINE_H
/* Included by backend.h after the storage/bridge inlines. */
static inline uint32_t g4_selection_width(uint64_t width) {
    if (width >= LLG_GMP_SUPPORTED_WIDTH_LIMIT)
        llg_gmp_sv4_fail("width reaches exclusive limit");
    return (uint32_t)width;
}
static inline uint32_t g4_selection_part_width(int64_t left, int64_t right) {
    uint64_t delta =
        left >= right ? (uint64_t)left - (uint64_t)right : (uint64_t)right - (uint64_t)left;
    if (delta == UINT64_MAX)
        llg_gmp_sv4_fail("width reaches exclusive limit");
    return g4_selection_width(delta + 1);
}
/* Fixed word permutations compile to shifts and a byte swap on common targets. */
static inline uint64_t g4_reverse_word(uint64_t word) {
    word =
        ((word >> 1) & UINT64_C(0x5555555555555555)) | ((word & UINT64_C(0x5555555555555555)) << 1);
    word =
        ((word >> 2) & UINT64_C(0x3333333333333333)) | ((word & UINT64_C(0x3333333333333333)) << 2);
    word =
        ((word >> 4) & UINT64_C(0x0f0f0f0f0f0f0f0f)) | ((word & UINT64_C(0x0f0f0f0f0f0f0f0f)) << 4);
    word =
        ((word >> 8) & UINT64_C(0x00ff00ff00ff00ff)) | ((word & UINT64_C(0x00ff00ff00ff00ff)) << 8);
    word = ((word >> 16) & UINT64_C(0x0000ffff0000ffff)) |
           ((word & UINT64_C(0x0000ffff0000ffff)) << 16);
    return (word >> 32) | (word << 32);
}
static inline int g4_selection_small_index(g4_t base, int64_t* index) {
    if (base.data.small.b)
        return 0;
    uint64_t word = base.data.small.a;
    if (base.is_signed && base.width && ((word >> (base.width - 1)) & 1))
        word |= ~g4_mask(base.width);
    else if (word > INT64_MAX)
        return 0;
    *index = word <= INT64_MAX ? (int64_t)word : -1 - (int64_t)~word;
    return 1;
}
static inline g4_t g4_selection_small_window(g4_t source, int64_t low, uint32_t width) {
    g4_t r = g4_small(UINT64_MAX, UINT64_MAX, width, 0);
    if (!width || low >= source.width || low <= -(int64_t)width)
        return r;
    uint32_t input = low < 0 ? 0 : (uint32_t)low;
    uint32_t output = low < 0 ? (uint32_t)-low : 0;
    uint32_t count = source.width - input;
    if (count > width - output)
        count = width - output;
    if (!count)
        return r;
    uint64_t mask = g4_mask(count) << output;
    r.data.small.a = (r.data.small.a & ~mask) | (((source.data.small.a >> input) << output) & mask);
    r.data.small.b = (r.data.small.b & ~mask) | (((source.data.small.b >> input) << output) & mask);
    return r;
}
static inline void g4_selection_small_write(g4_t* dst, int64_t low, g4_t source, int64_t source_low,
                                            uint32_t width) {
    if (!width || low >= dst->width || low <= -(int64_t)width)
        return;
    uint32_t skip = low < 0 ? (uint32_t)-low : 0;
    uint32_t start = (uint32_t)(low + skip), count = width - skip;
    if (count > dst->width - start)
        count = dst->width - start;
    /* Saturate before advancing a possibly extreme source coordinate. */
    if (source_low >= source.width || source_low <= -(int64_t)width)
        source_low = source.width;
    else
        source_low += skip;
    g4_t bits = g4_selection_small_window(source, source_low, count);
    uint64_t mask = g4_mask(count) << start;
    dst->data.small.a = (dst->data.small.a & ~mask) | (bits.data.small.a << start);
    dst->data.small.b = (dst->data.small.b & ~mask) | (bits.data.small.b << start);
}
static inline g4_t llg_gmp_sv4_bit_select(g4_t value, uint64_t index) {
    unsigned state = index >= value.width ? 2 : llg_gmp_sv4_state(value, (uint32_t)index);
    return g4_small(state == 1 || state == 2, state >= 2, 1, 0);
}
static inline void llg_gmp_sv4_bit_select_set(g4_t* dst, uint64_t index, g4_t source) {
    if (index < dst->width)
        llg_gmp_sv4_set_state(dst, (uint32_t)index, llg_gmp_sv4_state(source, 0));
}
static inline g4_t llg_gmp_sv4_part_select(g4_t source, int64_t left, int64_t right) {
    uint32_t width = g4_selection_part_width(left, right);
    if (source.width > 64 || width > 64)
        return llg_gmp_sv4_part_select_wide(source, left, right);
    if (left >= right)
        return g4_selection_small_window(source, right, width);
    g4_t r = g4_selection_small_window(source, left, width);
    r.data.small.a = g4_reverse_word(r.data.small.a) >> (64u - width);
    r.data.small.b = g4_reverse_word(r.data.small.b) >> (64u - width);
    return r;
}
static inline void llg_gmp_sv4_part_select_set(g4_t* dst, int64_t left, int64_t right,
                                               g4_t source) {
    uint32_t width = g4_selection_part_width(left, right);
    if (dst->width > 64 || source.width > 64) {
        llg_gmp_sv4_part_select_set_wide(dst, left, right, source);
        return;
    }
    if (left >= right) {
        g4_selection_small_write(dst, right, source, (int64_t)source.width - width, width);
        return;
    }
    if (!dst->width || right < 0 || left >= dst->width)
        return;
    int64_t start = left < 0 ? 0 : left,
            end = right >= dst->width ? (int64_t)dst->width - 1 : right;
    uint32_t count = (uint32_t)(end - start + 1);
    g4_t bits = g4_selection_small_window(source, (int64_t)source.width - (end - left + 1), count);
    bits.data.small.a = g4_reverse_word(bits.data.small.a) >> (64u - count);
    bits.data.small.b = g4_reverse_word(bits.data.small.b) >> (64u - count);
    g4_selection_small_write(dst, start, bits, 0, count);
}
static inline g4_t llg_gmp_sv4_idx_part_select(g4_t source, uint64_t base, uint32_t width,
                                               int negative) {
    g4_selection_width(width);
    if (source.width > 64 || width > 64)
        return llg_gmp_sv4_idx_part_select_wide(source, base, width, negative);
    if (base > INT64_MAX || !width)
        return g4_small(UINT64_MAX, UINT64_MAX, width, 0);
    return g4_selection_small_window(source, (int64_t)base - (negative ? (int64_t)width - 1 : 0),
                                     width);
}
static inline void llg_gmp_sv4_idx_part_select_set(g4_t* dst, uint64_t base, uint32_t width,
                                                   int negative, g4_t source) {
    g4_selection_width(width);
    if (dst->width > 64 || source.width > 64) {
        llg_gmp_sv4_idx_part_select_set_wide(dst, base, width, negative, source);
        return;
    }
    if (base <= INT64_MAX && width)
        g4_selection_small_write(dst, (int64_t)base - (negative ? (int64_t)width - 1 : 0), source,
                                 0, width);
}
static inline g4_t llg_gmp_sv4_idx_part_select_value(g4_t source, g4_t base, uint32_t width,
                                                     int negative) {
    g4_selection_width(width);
    if (source.width > 64 || base.width > 64 || width > 64)
        return llg_gmp_sv4_idx_part_select_value_wide(source, base, width, negative);
    int64_t index;
    if (!width || !g4_selection_small_index(base, &index) || (negative && index < 0))
        return g4_small(UINT64_MAX, UINT64_MAX, width, 0);
    return g4_selection_small_window(source, index - (negative ? (int64_t)width - 1 : 0), width);
}
static inline void llg_gmp_sv4_idx_part_select_set_value(g4_t* dst, g4_t base, uint32_t width,
                                                         int negative, g4_t source) {
    g4_selection_width(width);
    if (dst->width > 64 || source.width > 64 || base.width > 64) {
        llg_gmp_sv4_idx_part_select_set_value_wide(dst, base, width, negative, source);
        return;
    }
    int64_t index;
    if (width && g4_selection_small_index(base, &index) && !(negative && index < 0))
        g4_selection_small_write(dst, index - (negative ? (int64_t)width - 1 : 0), source, 0,
                                 width);
}
static inline g4_t llg_gmp_sv4_concat(g4_t hi, g4_t lo) {
    uint32_t width = g4_selection_width((uint64_t)hi.width + lo.width);
    if (width > 64)
        return llg_gmp_sv4_concat_wide(hi, lo);
    return g4_small(lo.data.small.a | (hi.width ? hi.data.small.a << lo.width : 0),
                    lo.data.small.b | (hi.width ? hi.data.small.b << lo.width : 0), width, 0);
}
static inline g4_t llg_gmp_sv4_repeat(g4_t pattern, uint64_t count) {
    if (pattern.width && count > UINT64_MAX / pattern.width)
        llg_gmp_sv4_fail("width reaches exclusive limit");
    uint32_t width = g4_selection_width((uint64_t)pattern.width * count);
    if (width > 64)
        return llg_gmp_sv4_repeat_wide(pattern, count);
    if (!width)
        return g4_small(0, 0, 0, 0);
    uint64_t a = pattern.data.small.a, b = pattern.data.small.b;
    for (uint32_t copied = pattern.width; copied < width;) {
        uint32_t take = width - copied < copied ? width - copied : copied;
        a |= (a & g4_mask(take)) << copied;
        b |= (b & g4_mask(take)) << copied;
        copied += take;
    }
    return g4_small(a, b, width, 0);
}
static inline g4_t llg_gmp_sv4_repeat_count(g4_t value) {
    if (value.width > 64)
        return llg_gmp_sv4_repeat_count_wide(value);
    if (value.data.small.b ||
        (value.is_signed && value.width && ((value.data.small.a >> (value.width - 1)) & 1)))
        return g4_small(0, 0, value.width, 0);
    value.is_signed = 0;
    return value;
}
static inline g4_t g4_selection_small_stream(g4_t v, uint32_t slice, int reverse, int inverse) {
    if (!slice)
        llg_gmp_sv4_fail("zero streaming slice size");
    if (!reverse || slice >= v.width) {
        v.is_signed = 0;
        return v;
    }
    g4_t r = g4_small(0, 0, v.width, 0);
    for (uint32_t bit = 0; bit < v.width;) {
        uint32_t count = v.width - bit < slice ? v.width - bit : slice;
        uint32_t other = v.width - bit - count;
        uint32_t src = inverse ? other : bit, dst = inverse ? bit : other;
        uint64_t mask = g4_mask(count);
        r.data.small.a |= ((v.data.small.a >> src) & mask) << dst;
        r.data.small.b |= ((v.data.small.b >> src) & mask) << dst;
        bit += count;
    }
    return r;
}
static inline g4_t llg_gmp_sv4_stream(g4_t v, uint32_t slice, int reverse) {
    return v.width <= 64 ? g4_selection_small_stream(v, slice, reverse, 0)
                         : llg_gmp_sv4_stream_wide(v, slice, reverse);
}
static inline g4_t llg_gmp_sv4_unstream(g4_t v, uint32_t slice, int reverse) {
    return v.width <= 64 ? g4_selection_small_stream(v, slice, reverse, 1)
                         : llg_gmp_sv4_unstream_wide(v, slice, reverse);
}
static inline g4_t llg_gmp_sv4_array_conditional_merge(g4_t a, g4_t b, g4_t def) {
    if (a.width > 64)
        return llg_gmp_sv4_array_conditional_merge_wide(a, b, def);
    uint32_t stride = def.width;
    if (!a.width || a.width != b.width || !stride || stride > a.width || a.width % stride)
        llg_gmp_sv4_fail("invalid array conditional merge shape");
    if (stride == 1) {
        uint64_t different = (a.data.small.a ^ b.data.small.a) | a.data.small.b | b.data.small.b;
        return g4_small((a.data.small.a & ~different) | (def.data.small.a ? different : 0),
                        def.data.small.b ? different : 0, a.width, 0);
    }
    g4_t r = g4_small(0, 0, a.width, 0);
    uint64_t mask = g4_mask(stride);
    for (uint32_t offset = 0; offset < a.width; offset += stride) {
        uint64_t aa = (a.data.small.a >> offset) & mask, ba = (b.data.small.a >> offset) & mask;
        int equal = aa == ba && !(((a.data.small.b | b.data.small.b) >> offset) & mask);
        r.data.small.a |= (equal ? aa : def.data.small.a) << offset;
        r.data.small.b |= (equal ? 0 : def.data.small.b) << offset;
    }
    return r;
}
static inline void g4_selection_plan_check(const llg_gmp_sv4_select_plan_t* plan) {
    if (!plan || !plan->storage_width || !plan->width ||
        plan->storage_width >= LLG_GMP_SUPPORTED_WIDTH_LIMIT ||
        plan->width >= LLG_GMP_SUPPORTED_WIDTH_LIMIT || plan->storage_lsb > plan->storage_width ||
        plan->value_lsb > plan->width || plan->count > plan->storage_width - plan->storage_lsb ||
        plan->count > plan->width - plan->value_lsb)
        llg_gmp_sv4_fail("invalid packed selection plan");
}
static inline g4_t llg_gmp_sv4_select_plan_read(g4_t source,
                                                const llg_gmp_sv4_select_plan_t* plan) {
    g4_selection_plan_check(plan);
    if (source.width != plan->storage_width)
        llg_gmp_sv4_fail("packed selection storage width mismatch");
    if (source.width > 64 || plan->width > 64)
        return llg_gmp_sv4_select_plan_read_wide(source, plan);
    g4_t r = g4_small(UINT64_MAX, UINT64_MAX, plan->width, 0);
    if (plan->count) {
        uint64_t mask = g4_mask(plan->count) << plan->value_lsb;
        r.data.small.a = (r.data.small.a & ~mask) |
                         (((source.data.small.a >> plan->storage_lsb) << plan->value_lsb) & mask);
        r.data.small.b = (r.data.small.b & ~mask) |
                         (((source.data.small.b >> plan->storage_lsb) << plan->value_lsb) & mask);
    }
    return r;
}
static inline g4_t llg_gmp_sv4_select_plan_slice(g4_t source, const llg_gmp_sv4_select_plan_t* plan,
                                                 int reverse) {
    g4_selection_plan_check(plan);
    if (source.width != plan->width)
        llg_gmp_sv4_fail("packed selection source width mismatch");
    if (source.width > 64)
        return llg_gmp_sv4_select_plan_slice_wide(source, plan, reverse);
    if (!plan->count)
        return g4_small(0, 0, 0, 0);
    if (!reverse)
        return g4_small(source.data.small.a >> plan->value_lsb,
                        source.data.small.b >> plan->value_lsb, plan->count, 0);
    return llg_gmp_sv4_part_select(source, source.width - plan->value_lsb - plan->count,
                                   source.width - plan->value_lsb - 1);
}
static inline void llg_gmp_sv4_select_plan_set(g4_t* dst, const llg_gmp_sv4_select_plan_t* plan,
                                               g4_t source) {
    g4_selection_plan_check(plan);
    if (!dst || dst->width != plan->storage_width || source.width != plan->width)
        llg_gmp_sv4_fail("packed selection assignment width mismatch");
    if (dst->width > 64 || source.width > 64) {
        llg_gmp_sv4_select_plan_set_wide(dst, plan, source);
        return;
    }
    if (!plan->count)
        return;
    uint64_t mask = g4_mask(plan->count) << plan->storage_lsb;
    dst->data.small.a = (dst->data.small.a & ~mask) |
                        (((source.data.small.a >> plan->value_lsb) << plan->storage_lsb) & mask);
    dst->data.small.b = (dst->data.small.b & ~mask) |
                        (((source.data.small.b >> plan->value_lsb) << plan->storage_lsb) & mask);
}
#endif
