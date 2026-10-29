#include "internal.h"
#include <math.h>
#include <stdio.h>

/* Same conversions as legacy (decision S01-D1): an integral delay is a
 * 64-bit time value whose scaled tick count wraps modulo 2^64; a real delay
 * without a 64-bit tick count waits one tick, with a warning. */
uint64_t llg_gmp_sv4_delay_ticks(g4_t value, uint64_t unit_ticks) {
    if (llg_gmp_sv4_is_unknown(value))
        return 0;
    const uint64_t* a = g4_a(&value);
    int negative = value.is_signed && value.width &&
                   ((a[(value.width - 1u) / 64u] >> ((value.width - 1u) % 64u)) & 1u);
    uint64_t raw = llg_gmp_sv4_to_u64(value);
    if (negative && value.width < 64)
        raw |= ~g4_mask(value.width);
    return raw * unit_ticks;
}
static uint64_t unrepresentable_real_delay(double value) {
    if (isnan(value))
        fprintf(stderr, "llg: warning: real delay is NaN; using 1 tick\n");
    else if (isinf(value))
        fprintf(stderr, "llg: warning: real delay is infinite; using 1 tick\n");
    else
        fprintf(stderr,
                "llg: warning: real delay %.17g exceeds the 64-bit tick range; using 1 tick\n",
                value);
    return 1;
}
uint64_t llg_gmp_sv4_real_delay_ticks(double value, uint64_t unit_ticks, uint64_t precision_ticks) {
    if (!isfinite(value) || !unit_ticks || !precision_ticks)
        return unrepresentable_real_delay(value);
    // Local-precision rounding, then a negative result as 64-bit
    // two's-complement unsigned time.
    double rounded = round(value * ((double)unit_ticks / (double)precision_ticks));
    uint64_t local;
    if (rounded >= 0.0) {
        if (!isfinite(rounded) || rounded >= 18446744073709551616.0)
            return unrepresentable_real_delay(value);
        local = (uint64_t)rounded;
    } else {
        if (!isfinite(rounded) || rounded < -9223372036854775808.0)
            return unrepresentable_real_delay(value);
        local = UINT64_C(0) - (uint64_t)(-rounded);
    }
    if (local > UINT64_MAX / precision_ticks)
        return unrepresentable_real_delay(value);
    return local * precision_ticks;
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
    if (magnitude == 0.0)
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
