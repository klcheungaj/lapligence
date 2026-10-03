#include "internal.h"

static void shift_plane(uint64_t* out, const uint64_t* in, uint32_t width, uint32_t count,
                        int right, uint64_t fill) {
    size_t n = ((size_t)width + 63u) / 64u, whole = count / 64u;
    unsigned bits = count % 64u;
    for (size_t i = 0; i < n; ++i) {
        uint64_t word = 0;
        if (right) {
            size_t source = i + whole;
            uint64_t low = source < n ? in[source] : fill;
            if (source + 1 == n)
                low |= fill & ~g4_topmask(width);
            word = low >> bits;
            if (bits) {
                uint64_t high = source + 1 < n ? in[source + 1] : fill;
                if (source + 2 == n)
                    high |= fill & ~g4_topmask(width);
                word |= high << (64u - bits);
            }
        } else if (i >= whole) {
            size_t source = i - whole;
            word = in[source] << bits;
            if (bits && source)
                word |= in[source - 1] >> (64u - bits);
        }
        out[i] = word;
    }
    out[n - 1] &= g4_topmask(width);
}
g4_t llg_gmp_sv4_shift_wide(g4_t value, g4_t count, int right, int arithmetic) {
    if (llg_gmp_sv4_is_unknown(count))
        return llg_gmp_sv4_x(value.width, value.is_signed);
    const uint64_t* cp = g4_a(&count);
    size_t cn = llg_gmp_sv4_words(count);
    int oversized = 0;
    for (size_t i = 1; i < cn; ++i)
        oversized |= cp[i] != 0;
    uint64_t shift = cn ? cp[0] : 0;
    if (oversized || shift >= value.width) {
        uint8_t state = right && arithmetic && value.is_signed && value.width
                            ? llg_gmp_sv4_state(value, value.width - 1u)
                            : 0;
        return llg_gmp_sv4_fill(state, value.width, value.is_signed);
    }
    if (value.width <= 64)
        return g4_shift(value, g4_small(shift, 0, 64, 0), right, arithmetic);
    if (!shift)
        return llg_gmp_sv4_clone(&value);
    size_t n = llg_gmp_sv4_words(value);
    const uint64_t *a = value.data.wide.a, *b = value.data.wide.b;
    unsigned top = (value.width - 1u) % 64u;
    uint64_t fa =
        right && arithmetic && value.is_signed && ((a[n - 1] >> top) & 1) ? UINT64_MAX : 0;
    uint64_t fb =
        right && arithmetic && value.is_signed && b && ((b[n - 1] >> top) & 1) ? UINT64_MAX : 0;
    g4_t out = llg_gmp_sv4_new(value.width, value.is_signed, b != NULL);
    shift_plane(out.data.wide.a, a, value.width, (uint32_t)shift, right, fa);
    if (b)
        shift_plane(out.data.wide.b, b, value.width, (uint32_t)shift, right, fb);
    llg_gmp_sv4_finish(&out);
    return out;
}
g4_t llg_gmp_sv4_reduce_wide(g4_t value, unsigned op) {
    const uint64_t *a = value.data.wide.a, *b = value.data.wide.b;
    size_t n = llg_gmp_sv4_words(value);
    unsigned kind = op % 3u, parity = 0;
    int unknown = b != NULL;
    if (kind == 2 && unknown)
        return g4_predicate(2);
    for (size_t i = 0; i < n; ++i) {
        uint64_t mask = i + 1 == n ? g4_topmask(value.width) : UINT64_MAX;
        uint64_t u = b ? b[i] : 0;
        if ((kind == 0 && (~a[i] & ~u & mask)) || (kind == 1 && (a[i] & ~u)))
            return g4_predicate(op >= 3 ? kind == 0 : kind == 1);
        if (kind == 2)
            parity ^= g4_popcount(a[i]) & 1u;
    }
    int truth = unknown ? 2 : kind == 0 ? 1 : kind == 1 ? 0 : (int)parity;
    return g4_predicate(op >= 3 && truth != 2 ? !truth : truth);
}
g4_t llg_gmp_sv4_countones_wide(g4_t value) {
    const uint64_t *a = value.data.wide.a, *b = value.data.wide.b;
    size_t n = llg_gmp_sv4_words(value);
    uint32_t count = 0;
    for (size_t i = 0; i < n; ++i)
        count += g4_popcount(a[i] & ~(b ? b[i] : 0));
    return g4_small(count, 0, 32, 1);
}
g4_t llg_gmp_sv4_onehot_wide(g4_t value, int allow_zero) {
    const uint64_t *a = value.data.wide.a, *b = value.data.wide.b;
    size_t n = llg_gmp_sv4_words(value);
    int found = 0;
    for (size_t i = 0; i < n; ++i) {
        uint64_t ones = a[i] & ~(b ? b[i] : 0);
        if (!ones)
            continue;
        if (found || (ones & (ones - 1u)))
            return g4_predicate(0);
        found = 1;
    }
    return g4_predicate(found || allow_zero);
}
