// llg_random.c — IEEE Verilog probabilistic distribution algorithms.

#include "llg_random.h"

#include <float.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

_Static_assert(sizeof(float) == sizeof(uint32_t),
               "legacy random algorithm requires a 32-bit float");
_Static_assert(FLT_RADIX == 2 && FLT_MANT_DIG == 24 && FLT_MAX_EXP == 128,
               "legacy random algorithm requires IEEE binary32 float");

enum {
    LLG_RANDOM_DEFAULT_SEED = 259341593,
};

static int32_t bits_to_i32(uint32_t bits) {
    int32_t value;
    memcpy(&value, &bits, sizeof(value));
    return value;
}

static uint32_t i32_to_bits(int32_t value) {
    uint32_t bits;
    memcpy(&bits, &value, sizeof(bits));
    return bits;
}

// The standard's reference code casts a real to long after adding 0.5.  A
// generated model has a signed 32-bit result regardless of the host's long
// width; clamp before the conversion so a pathological distribution tail
// cannot invoke an out-of-range floating-to-integer conversion.
static int32_t rounded_i32(double value) {
    if (!isfinite(value)) return 0;
    double rounded = value >= 0.0 ? floor(value + 0.5) : -floor(-value + 0.5);
    if (rounded <= -2147483648.0) return INT32_MIN;
    if (rounded >= 2147483647.0) return INT32_MAX;
    return (int32_t)rounded;
}

// Match the reference code's conversion of a continuous value into an
// integer interval.  The negative branch intentionally subtracts one before
// truncation, which is the specified Annex N source rather than floor().
static int64_t interval_integer(double value) {
    if (value >= 0.0) return (int64_t)value;
    return (int64_t)ceil(value - 1.0);
}

static int32_t clamp_interval_i32(int64_t value) {
    if (value <= INT32_MIN) return INT32_MIN;
    if (value >= INT32_MAX) return INT32_MAX;
    return (int32_t)value;
}

static int valid_seed(int32_t *seed) {
    if (seed != NULL) return 1;
    fputs("llg random warning: seed must be a writable 32-bit value\n", stderr);
    return 0;
}

static double uniform(int32_t *seed, int64_t start, int64_t end) {
    const double correction = 0.00000011920928955078125; // 2^-23
    double a;
    double b;
    if (*seed == 0) *seed = LLG_RANDOM_DEFAULT_SEED;
    if (start >= end) {
        a = 0.0;
        b = 2147483647.0;
    } else {
        a = (double)start;
        b = (double)end;
    }

    // 69069 * seed + 1 is intentionally modulo 2^32.  Unsigned arithmetic
    // makes the wrap explicit and avoids the signed-overflow assumption in
    // the historical C listing.
    uint32_t state = UINT32_C(69069) * i32_to_bits(*seed) + UINT32_C(1);
    *seed = bits_to_i32(state);

    uint32_t float_bits = (state >> 9) | UINT32_C(0x3f800000);
    float sample;
    memcpy(&sample, &float_bits, sizeof(sample));
    double c = (double)sample;
    c += c * correction;
    return (b - a) * (c - 1.0) + a;
}

static double normal(int32_t *seed, int32_t mean, int32_t deviation) {
    double v1;
    double v2;
    double s = 1.0;
    while (s >= 1.0 || s == 0.0) {
        v1 = uniform(seed, -1, 1);
        v2 = uniform(seed, -1, 1);
        s = v1 * v1 + v2 * v2;
    }
    s = v1 * sqrt(-2.0 * log(s) / s);
    return s * (double)deviation + (double)mean;
}

static double exponential(int32_t *seed, int32_t mean) {
    double n = uniform(seed, 0, 1);
    if (n != 0.0) n = -log(n) * (double)mean;
    return n;
}

static int32_t poisson(int32_t *seed, int32_t mean) {
    int64_t n = 0;
    double p = exp(-(double)mean);
    double q = uniform(seed, 0, 1);
    while (p < q) {
        if (n == INT32_MAX) break;
        ++n;
        q = uniform(seed, 0, 1) * q;
    }
    return n > INT32_MAX ? INT32_MAX : (int32_t)n;
}

static double chi_square(int32_t *seed, int32_t degree_of_freedom) {
    double x;
    if (degree_of_freedom % 2) {
        x = normal(seed, 0, 1);
        x *= x;
    } else {
        x = 0.0;
    }
    // Use a wider loop counter so df=INT32_MAX cannot wrap the increment.
    for (int64_t k = 2; k <= degree_of_freedom; k += 2)
        x += 2.0 * exponential(seed, 1);
    return x;
}

static double student_t(int32_t *seed, int32_t degree_of_freedom) {
    double chi2 = chi_square(seed, degree_of_freedom);
    double root = sqrt(chi2 / (double)degree_of_freedom);
    return normal(seed, 0, 1) / root;
}

static double erlangian(int32_t *seed, int32_t k_stage, int32_t mean) {
    double x = 1.0;
    for (int64_t i = 1; i <= k_stage; ++i) x *= uniform(seed, 0, 1);
    return -(double)mean * log(x) / (double)k_stage;
}

int32_t llg_dist_uniform(int32_t *seed, int32_t start, int32_t end) {
    if (!valid_seed(seed)) return 0;
    if (start >= end) return start;
    if (end != INT32_MAX) {
        int64_t upper = (int64_t)end + 1;
        int64_t value = interval_integer(uniform(seed, start, upper));
        if (value < start) value = start;
        if (value >= upper) value = upper - 1;
        return (int32_t)value;
    }
    if (start != INT32_MIN) {
        int64_t lower = (int64_t)start - 1;
        int64_t value = interval_integer(uniform(seed, lower, end) + 1.0);
        if (value <= lower) value = lower + 1;
        if (value > end) value = end;
        return (int32_t)value;
    }
    double value = (uniform(seed, INT32_MIN, INT32_MAX) + 2147483648.0) /
                   4294967295.0;
    value = value * 4294967296.0 - 2147483648.0;
    return clamp_interval_i32(interval_integer(value));
}

int32_t llg_dist_normal(int32_t *seed, int32_t mean, int32_t deviation) {
    if (!valid_seed(seed)) return 0;
    return rounded_i32(normal(seed, mean, deviation));
}

int32_t llg_dist_exponential(int32_t *seed, int32_t mean) {
    if (!valid_seed(seed)) return 0;
    if (mean <= 0) {
        fputs("llg random warning: exponential distribution requires a positive mean\n",
              stderr);
        return 0;
    }
    return rounded_i32(exponential(seed, mean));
}

int32_t llg_dist_poisson(int32_t *seed, int32_t mean) {
    if (!valid_seed(seed)) return 0;
    if (mean <= 0) {
        fputs("llg random warning: poisson distribution requires a positive mean\n",
              stderr);
        return 0;
    }
    return poisson(seed, mean);
}

int32_t llg_dist_chi_square(int32_t *seed, int32_t degree_of_freedom) {
    if (!valid_seed(seed)) return 0;
    if (degree_of_freedom <= 0) {
        fputs("llg random warning: chi-square distribution requires a positive degree of freedom\n",
              stderr);
        return 0;
    }
    return rounded_i32(chi_square(seed, degree_of_freedom));
}

int32_t llg_dist_t(int32_t *seed, int32_t degree_of_freedom) {
    if (!valid_seed(seed)) return 0;
    if (degree_of_freedom <= 0) {
        fputs("llg random warning: t distribution requires a positive degree of freedom\n",
              stderr);
        return 0;
    }
    return rounded_i32(student_t(seed, degree_of_freedom));
}

int32_t llg_dist_erlang(int32_t *seed, int32_t k_stage, int32_t mean) {
    if (!valid_seed(seed)) return 0;
    if (k_stage <= 0) {
        fputs("llg random warning: Erlang distribution requires a positive k-stage\n",
              stderr);
        return 0;
    }
    return rounded_i32(erlangian(seed, k_stage, mean));
}

int32_t llg_random_next(int32_t *seed) {
    return llg_dist_uniform(seed, INT32_MIN, INT32_MAX);
}

int32_t llg_random_default(void) {
    static int32_t seed = LLG_RANDOM_DEFAULT_SEED;
    return llg_random_next(&seed);
}
