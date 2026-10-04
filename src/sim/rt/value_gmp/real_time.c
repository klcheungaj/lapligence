#include "internal.h"
#include <math.h>

static uint64_t checked_product(uint64_t value, uint64_t scale) {
    if (!scale || value > UINT64_MAX / scale)
        llg_gmp_sv4_fail("delay exceeds the 64-bit tick range");
    return value * scale;
}
uint64_t llg_gmp_sv4_delay_ticks(g4_t value, uint64_t unit_ticks) {
    if (llg_gmp_sv4_is_unknown(value))
        return 0;
    const uint64_t* a = g4_a(&value);
    int negative = value.is_signed && value.width &&
                   ((a[(value.width - 1u) / 64u] >> ((value.width - 1u) % 64u)) & 1u);
    uint64_t raw = llg_gmp_sv4_to_u64(value);
    if (negative && value.width < 64)
        raw |= ~g4_mask(value.width);
    if (!negative)
        for (size_t i = 1; i < llg_gmp_sv4_words(value); ++i)
            if (a[i])
                llg_gmp_sv4_fail("delay exceeds the 64-bit tick range");
    return checked_product(raw, unit_ticks);
}
uint64_t llg_gmp_sv4_real_delay_ticks(double value, uint64_t unit_ticks, uint64_t precision_ticks) {
    if (!isfinite(value) || !unit_ticks || !precision_ticks)
        llg_gmp_sv4_fail("real delay must be finite");
    // Same conversion as legacy: local-precision rounding, then a negative
    // result as 64-bit two's-complement unsigned time.
    double rounded = round(value * ((double)unit_ticks / (double)precision_ticks));
    uint64_t local;
    if (rounded >= 0.0) {
        if (!isfinite(rounded) || rounded >= 18446744073709551616.0)
            llg_gmp_sv4_fail("delay exceeds the 64-bit tick range");
        local = (uint64_t)rounded;
    } else {
        if (!isfinite(rounded) || rounded < -9223372036854775808.0)
            llg_gmp_sv4_fail("delay exceeds the 64-bit tick range");
        local = UINT64_C(0) - (uint64_t)(-rounded);
    }
    return checked_product(local, precision_ticks);
}

double llg_gmp_sv4_to_real_wide(g4_t value) {
    const uint64_t *a = g4_a(&value), *b = g4_b(&value);
    size_t n = llg_gmp_sv4_words(value);
    uint64_t sign = UINT64_C(1) << ((value.width - 1u) % 64u);
    int negative = value.is_signed && (a[n - 1u] & ~(b ? b[n - 1u] : 0) & sign);
    size_t first = 0;
    if (negative)
        while (first < n && !(a[first] & ~(b ? b[first] : 0)))
            ++first;
    double out = 0.0;
    for (size_t i = n; i-- > 0;) {
        uint64_t word = a[i] & ~(b ? b[i] : 0);
        if (negative)
            word = ~word + (uint64_t)(i <= first);
        if (i + 1u == n)
            word &= g4_topmask(value.width);
        out = ldexp(out, 64) + (double)word;
    }
    return negative ? -out : out;
}
g4_t llg_gmp_sv4_from_real_wide(double rounded, uint32_t width, int8_t sign) {
    g4_t out = llg_gmp_sv4_zero(width, sign);
    double magnitude = fabs(rounded);
    if (!magnitude)
        return out;
    int exponent;
    double fraction = frexp(magnitude, &exponent);
    uint64_t significand = (uint64_t)ldexp(fraction, 53);
    int shift = exponent - 53;
    if (shift < 0) {
        significand >>= (unsigned)-shift;
        shift = 0;
    }
    uint64_t* a = g4_mut_a(&out);
    size_t n = llg_gmp_sv4_words(out), word = (unsigned)shift / 64u;
    unsigned bit = (unsigned)shift % 64u;
    if (word < n) {
        a[word] = significand << bit;
        if (bit && word + 1u < n)
            a[word + 1u] = significand >> (64u - bit);
    }
    if (signbit(rounded)) {
        uint64_t carry = 1;
        for (size_t i = 0; i < n; ++i) {
            a[i] = ~a[i] + carry;
            carry &= a[i] == 0;
        }
    }
    llg_gmp_sv4_finish(&out);
    return out;
}
