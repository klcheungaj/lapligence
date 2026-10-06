#include "internal.h"

static uint64_t extract(const uint64_t* plane, uint32_t width, uint32_t offset) {
    if (!plane || offset >= width)
        return 0;
    size_t word = offset / 64u;
    unsigned shift = offset % 64u;
    uint64_t result = plane[word] >> shift;
    if (shift && word + 1u < ((size_t)width + 63u) / 64u)
        result |= plane[word + 1u] << (64u - shift);
    return result & g4_mask(width - offset);
}

static void store_word(g4_t* out, size_t i, uint64_t a, uint64_t b) {
    if (b && out->width > 64)
        llg_gmp_sv4_promote(out);
    g4_mut_a(out)[i] = a;
    uint64_t* plane = g4_mut_b(out);
    if (plane)
        plane[i] = b;
}
static g4_t resolve(const g4_t* const* drivers, const uint8_t* strength0,
                     const uint8_t* strength1, const int* indices, int count,
                     uint32_t offset, uint32_t width, int8_t sign, int mode) {
    int strengths = strength0 && strength1;
    /* Most resolved nets are known: start with the A plane only and promote B
     * once, at the first word that carries X/Z, instead of allocating both
     * planes and shrinking the canonical result afterwards. */
    g4_t out = llg_gmp_sv4_new(width, sign, 0);
    size_t n = llg_gmp_sv4_words(out);
    for (size_t i = 0; i < n; ++i) {
        uint64_t mask = i + 1u == n ? g4_topmask(width) : UINT64_MAX;
        uint32_t source = offset + (uint32_t)i * 64u;
        uint64_t known0[8] = {0}, known1[8] = {0}, possible0[8] = {0}, possible1[8] = {0};
        uint64_t any0 = 0, any1 = 0, anyx = 0;
        if (!strengths && (mode == LLG_GMP_RESOLVE_SUPPLY0 || mode == LLG_GMP_RESOLVE_SUPPLY1)) {
            store_word(&out, i, mode == LLG_GMP_RESOLVE_SUPPLY1 ? mask : 0, 0);
            continue;
        }
        if (strengths && mode >= LLG_GMP_RESOLVE_TRI0 && mode <= LLG_GMP_RESOLVE_SUPPLY1) {
            unsigned level = mode <= LLG_GMP_RESOLVE_TRI1 ? LLG_GMP_STRENGTH_PULL
                                                          : LLG_GMP_STRENGTH_SUPPLY;
            if (mode == LLG_GMP_RESOLVE_TRI0 || mode == LLG_GMP_RESOLVE_SUPPLY0)
                known0[level] = possible0[level] = mask;
            else
                known1[level] = possible1[level] = mask;
        }
        for (int selected = 0; selected < count; ++selected) {
            int d = indices ? indices[selected] : selected;
            if (d < 0)
                llg_gmp_sv4_fail("invalid net driver index");
            const g4_t* v = drivers[d];
            if (!v)
                continue;
            uint64_t valid = source >= v->width ? 0 : g4_mask(v->width - source);
            uint64_t aa = extract(g4_a(v), v->width, source);
            uint64_t bb = extract(g4_b(v), v->width, source);
            uint64_t x = aa & bb & mask;
            uint64_t k0 = ~aa & ~bb & valid & mask, k1 = aa & ~bb & mask;
            if (!strengths) {
                any0 |= k0;
                any1 |= k1;
                anyx |= x;
            } else {
                unsigned s0 = strength0[d], s1 = strength1[d];
                if (s0 > LLG_GMP_STRENGTH_SUPPLY || s1 > LLG_GMP_STRENGTH_SUPPLY)
                    llg_gmp_sv4_fail("invalid net drive strength");
                if (s0) {
                    known0[s0] |= k0;
                    possible0[s0] |= k0 | x;
                }
                if (s1) {
                    known1[s1] |= k1;
                    possible1[s1] |= k1 | x;
                }
            }
        }
        uint64_t ones, unknown, absent;
        if (!strengths) {
            absent = ~(any0 | any1 | anyx) & mask;
            if (mode == LLG_GMP_RESOLVE_WAND) {
                ones = any1 & ~anyx & ~any0;
                unknown = anyx & ~any0;
            } else if (mode == LLG_GMP_RESOLVE_WOR) {
                ones = any1;
                unknown = anyx & ~any1;
            } else {
                unknown = anyx | (any0 & any1);
                ones = any1 & ~unknown;
            }
            if (mode == LLG_GMP_RESOLVE_TRI0 || mode == LLG_GMP_RESOLVE_TRI1) {
                if (mode == LLG_GMP_RESOLVE_TRI1)
                    ones |= absent;
                absent = 0;
            }
        } else {
            uint64_t stronger0 = 0, stronger1 = 0, zeros = 0;
            ones = 0;
            /* Opposite endpoints at greater strength prevent a known result.
             * Only the wired mode's dominant value may also win an equal tie. */
            for (unsigned s = LLG_GMP_STRENGTH_SUPPLY; s; --s) {
                zeros |= known0[s] & ~stronger1 &
                         (mode == LLG_GMP_RESOLVE_WAND ? UINT64_MAX : ~possible1[s]);
                ones |= known1[s] & ~stronger0 &
                        (mode == LLG_GMP_RESOLVE_WOR ? UINT64_MAX : ~possible0[s]);
                stronger0 |= possible0[s];
                stronger1 |= possible1[s];
            }
            absent = ~(stronger0 | stronger1) & mask;
            unknown = ~(zeros | ones | absent) & mask;
        }
        store_word(&out, i, (ones | unknown) & mask, (unknown | absent) & mask);
    }
    llg_gmp_sv4_finish(&out);
    return out;
}

g4_t llg_gmp_sv4_resolve(const g4_t* const* drivers, int count, uint32_t width,
                          int8_t sign, int mode) {
    return resolve(drivers, NULL, NULL, NULL, count, 0, width, sign, mode);
}
g4_t llg_gmp_sv4_resolve_strengths(const g4_t* const* drivers, const uint8_t* strength0,
                                    const uint8_t* strength1, int count, uint32_t width,
                                    int8_t sign, int mode) {
    return resolve(drivers, strength0, strength1, NULL, count, 0, width, sign, mode);
}
g4_t llg_gmp_sv4_resolve_strengths_range(const g4_t* const* drivers, const uint8_t* strength0,
                                          const uint8_t* strength1, const int* indices, int count,
                                          uint32_t source_width, uint32_t offset, uint32_t width,
                                          int8_t sign, int mode) {
    g4_width_check(source_width);
    g4_width_check(width);
    if ((!indices && count) || count < 0 || offset > source_width || width > source_width - offset)
        llg_gmp_sv4_fail("invalid net resolution range");
    return resolve(drivers, strength0, strength1, indices, count, offset, width, sign, mode);
}
g4_t llg_gmp_sv4_enum_navigate(g4_t current, g4_t step, const g4_t* values, uint32_t count,
                                g4_t default_value, int direction) {
    if (!values || !count)
        return llg_gmp_sv4_clone(&default_value);
    uint32_t found = count;
    for (uint32_t i = 0; i < count; ++i) {
        g4_t match = llg_gmp_sv4_case_eq(current, values[i]);
        if (llg_gmp_sv4_to_bool(match))
            found = i;
    }
    if (found == count)
        return llg_gmp_sv4_clone(&default_value);
    uint64_t distance = llg_gmp_sv4_is_unknown(step) ? 0 : llg_gmp_sv4_to_u64(step);
    uint64_t offset = distance % count;
    uint32_t target = (uint32_t)((direction < 0 ? (uint64_t)found + count - offset
                                               : (uint64_t)found + offset) % count);
    return llg_gmp_sv4_clone(&values[target]);
}
g4_t llg_gmp_sv4_udp_eval(const uint8_t* rows, size_t row_count, size_t input_count,
                           const g4_t* const* inputs) {
    size_t stride = input_count + 1u;
    for (size_t row = 0; row < row_count; ++row) {
        const uint8_t* fields = rows + row * stride;
        size_t input = 0;
        for (; input < input_count; ++input) {
            unsigned state = llg_gmp_sv4_state(*inputs[input], 0);
            if (state == 3)
                state = 2;
            if (!(fields[input] & (1u << state)))
                break;
        }
        if (input == input_count)
            return llg_gmp_sv4_fill(fields[input_count], 1, 0);
    }
    return llg_gmp_sv4_x(1, 0);
}
