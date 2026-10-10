/*
 * Independent oracle for annex_n.sv: a transcription of the normative C
 * source in IEEE Std 1800-2009 Annex N.2 (identical to IEEE Std 1364-2001
 * 17.9.3), written from the standard's text and not from the llg runtime.
 *
 * Adaptations, each forced by portable C11 and none changing a result:
 * - The standard's `long` is the 32-bit Verilog integer, so `rtl_long` is
 *   int32_t and LONG_MIN/LONG_MAX are INT32_MIN/INT32_MAX.
 * - `69069 * (*seed) + 1` wraps modulo 2^32 in the standard's 32-bit long;
 *   it is computed in uint32_t because signed overflow is undefined in C.
 * - Every `(long)` conversion of a double goes through to_long(), which
 *   aborts instead of invoking undefined behaviour if a vector ever leaves
 *   the 32-bit range (none of the vectors below does).
 * - The listing's third rtl_dist_uniform branch has an unbalanced
 *   parenthesis; the only well-formed reading is used.
 * - print_error() writes its two message parts to stderr.
 * - K&R definitions are written as prototypes.
 *
 * The program prints, in order, the lines annex_n.sv displays.
 */
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

typedef int32_t rtl_long;
#define RTL_LONG_MIN INT32_MIN
#define RTL_LONG_MAX INT32_MAX

static void print_error(const char* first, const char* second) {
    fputs(first, stderr);
    fputs(second, stderr);
}

static rtl_long to_long(double value) {
    if (!(value > -2147483649.0 && value < 2147483648.0)) {
        fprintf(stderr, "reference vector left the 32-bit range\n");
        abort();
    }
    return (rtl_long)value;
}

static double uniform(rtl_long* seed, rtl_long start, rtl_long end);
static double normal(rtl_long* seed, rtl_long mean, rtl_long deviation);
static double exponential(rtl_long* seed, rtl_long mean);
static rtl_long poisson(rtl_long* seed, rtl_long mean);
static double chi_square(rtl_long* seed, rtl_long deg_of_free);
static double t(rtl_long* seed, rtl_long deg_of_free);
static double erlangian(rtl_long* seed, rtl_long k, rtl_long mean);

static rtl_long rtl_dist_chi_square(rtl_long* seed, rtl_long df) {
    double r;
    rtl_long i;
    if (df > 0) {
        r = chi_square(seed, df);
        if (r >= 0) {
            i = to_long(r + 0.5);
        } else {
            r = -r;
            i = to_long(r + 0.5);
            i = -i;
        }
    } else {
        print_error("WARNING: Chi_square distribution must ",
                    "have positive degree of freedom\n");
        i = 0;
    }
    return (i);
}

static rtl_long rtl_dist_erlang(rtl_long* seed, rtl_long k, rtl_long mean) {
    double r;
    rtl_long i;
    if (k > 0) {
        r = erlangian(seed, k, mean);
        if (r >= 0) {
            i = to_long(r + 0.5);
        } else {
            r = -r;
            i = to_long(r + 0.5);
            i = -i;
        }
    } else {
        print_error("WARNING: k-stage erlangian distribution ",
                    "must have positive k\n");
        i = 0;
    }
    return (i);
}

static rtl_long rtl_dist_exponential(rtl_long* seed, rtl_long mean) {
    double r;
    rtl_long i;
    if (mean > 0) {
        r = exponential(seed, mean);
        if (r >= 0) {
            i = to_long(r + 0.5);
        } else {
            r = -r;
            i = to_long(r + 0.5);
            i = -i;
        }
    } else {
        print_error("WARNING: Exponential distribution must ",
                    "have a positive mean\n");
        i = 0;
    }
    return (i);
}

static rtl_long rtl_dist_normal(rtl_long* seed, rtl_long mean, rtl_long sd) {
    double r;
    rtl_long i;
    r = normal(seed, mean, sd);
    if (r >= 0) {
        i = to_long(r + 0.5);
    } else {
        r = -r;
        i = to_long(r + 0.5);
        i = -i;
    }
    return (i);
}

static rtl_long rtl_dist_poisson(rtl_long* seed, rtl_long mean) {
    rtl_long i;
    if (mean > 0) {
        i = poisson(seed, mean);
    } else {
        print_error("WARNING: Poisson distribution must have a ",
                    "positive mean\n");
        i = 0;
    }
    return (i);
}

static rtl_long rtl_dist_t(rtl_long* seed, rtl_long df) {
    double r;
    rtl_long i;
    if (df > 0) {
        r = t(seed, df);
        if (r >= 0) {
            i = to_long(r + 0.5);
        } else {
            r = -r;
            i = to_long(r + 0.5);
            i = -i;
        }
    } else {
        print_error("WARNING: t distribution must have positive ",
                    "degree of freedom\n");
        i = 0;
    }
    return (i);
}

static rtl_long rtl_dist_uniform(rtl_long* seed, rtl_long start, rtl_long end) {
    double r;
    rtl_long i;
    if (start >= end) return (start);
    if (end != RTL_LONG_MAX) {
        end++;
        r = uniform(seed, start, end);
        if (r >= 0) {
            i = to_long(r);
        } else {
            i = to_long(r - 1);
        }
        if (i < start) i = start;
        if (i >= end) i = end - 1;
    } else if (start != RTL_LONG_MIN) {
        start--;
        r = uniform(seed, start, end) + 1.0;
        if (r >= 0) {
            i = to_long(r);
        } else {
            i = to_long(r - 1);
        }
        if (i <= start) i = start + 1;
        if (i > end) i = end;
    } else {
        r = (uniform(seed, start, end) + 2147483648.0) / 4294967295.0;
        r = r * 4294967296.0 - 2147483648.0;
        if (r >= 0) {
            i = to_long(r);
        } else {
            i = to_long(r - 1);
        }
    }
    return (i);
}

static double uniform(rtl_long* seed, rtl_long start, rtl_long end) {
    union u_s {
        float s;
        uint32_t stemp;
    } u;
    double d = 0.00000011920928955078125;
    double a, b, c;
    if ((*seed) == 0) *seed = 259341593;
    if (start >= end) {
        a = 0.0;
        b = 2147483647.0;
    } else {
        a = (double)start;
        b = (double)end;
    }
    *seed = (rtl_long)(UINT32_C(69069) * (uint32_t)(*seed) + UINT32_C(1));
    u.stemp = (uint32_t)*seed;
    /*
     * This relies on IEEE floating point format
     */
    u.stemp = (u.stemp >> 9) | 0x3f800000;
    c = (double)u.s;
    c = c + (c * d);
    c = ((b - a) * (c - 1.0)) + a;
    return (c);
}

static double normal(rtl_long* seed, rtl_long mean, rtl_long deviation) {
    double v1 = 0.0, v2, s;
    s = 1.0;
    while ((s >= 1.0) || (s == 0.0)) {
        v1 = uniform(seed, -1, 1);
        v2 = uniform(seed, -1, 1);
        s = v1 * v1 + v2 * v2;
    }
    s = v1 * sqrt(-2.0 * log(s) / s);
    v1 = (double)deviation;
    v2 = (double)mean;
    return (s * v1 + v2);
}

static double exponential(rtl_long* seed, rtl_long mean) {
    double n;
    n = uniform(seed, 0, 1);
    if (n != 0) {
        n = -log(n) * mean;
    }
    return (n);
}

static rtl_long poisson(rtl_long* seed, rtl_long mean) {
    rtl_long n;
    double p, q;
    n = 0;
    q = -(double)mean;
    p = exp(q);
    q = uniform(seed, 0, 1);
    while (p < q) {
        n++;
        q = uniform(seed, 0, 1) * q;
    }
    return (n);
}

static double chi_square(rtl_long* seed, rtl_long deg_of_free) {
    double x;
    rtl_long k;
    if (deg_of_free % 2) {
        x = normal(seed, 0, 1);
        x = x * x;
    } else {
        x = 0.0;
    }
    for (k = 2; k <= deg_of_free; k = k + 2) {
        x = x + 2 * exponential(seed, 1);
    }
    return (x);
}

static double t(rtl_long* seed, rtl_long deg_of_free) {
    double x;
    double chi2 = chi_square(seed, deg_of_free);
    double div = chi2 / (double)deg_of_free;
    double root = sqrt(div);
    x = normal(seed, 0, 1) / root;
    return (x);
}

static double erlangian(rtl_long* seed, rtl_long k, rtl_long mean) {
    double x, a, b;
    rtl_long i;
    x = 1.0;
    for (i = 1; i <= k; i++) {
        x = x * uniform(seed, 0, 1);
    }
    a = (double)mean;
    b = (double)k;
    x = -a * log(x) / b;
    return (x);
}

/* The vectors annex_n.sv runs, in the same order. $random is
 * rtl_dist_uniform(seed, LONG_MIN, LONG_MAX) (Table N.1). */
static rtl_long seed;

static void show(const char* name, rtl_long value) {
    printf("%s %ld %ld\n", name, (long)value, (long)seed);
}

int main(void) {
    static const rtl_long random_seeds[] = {0, 1, -1, 12345, RTL_LONG_MIN,
                                            RTL_LONG_MAX};
    for (size_t n = 0; n < sizeof(random_seeds) / sizeof(random_seeds[0]); ++n) {
        seed = random_seeds[n];
        for (int k = 0; k < 3; ++k)
            show("random", rtl_dist_uniform(&seed, RTL_LONG_MIN, RTL_LONG_MAX));
    }
    seed = 7;
    for (int k = 0; k < 3; ++k) show("uniform", rtl_dist_uniform(&seed, 0, 10));
    seed = 7;
    for (int k = 0; k < 3; ++k) show("uniform", rtl_dist_uniform(&seed, -5, 5));
    seed = -99;
    for (int k = 0; k < 3; ++k)
        show("uniform", rtl_dist_uniform(&seed, RTL_LONG_MIN, RTL_LONG_MAX - 1));
    seed = 3;
    for (int k = 0; k < 3; ++k)
        show("uniform", rtl_dist_uniform(&seed, 100, RTL_LONG_MAX));
    seed = 3;
    for (int k = 0; k < 3; ++k)
        show("uniform", rtl_dist_uniform(&seed, RTL_LONG_MIN, -100));
    seed = 11;
    show("uniform", rtl_dist_uniform(&seed, 5, 5));
    show("uniform", rtl_dist_uniform(&seed, 5, -5));
    seed = 10;
    for (int k = 0; k < 3; ++k) show("normal", rtl_dist_normal(&seed, 10, 2));
    seed = 21;
    for (int k = 0; k < 3; ++k) show("normal", rtl_dist_normal(&seed, 0, 100));
    seed = -5;
    for (int k = 0; k < 2; ++k) show("normal", rtl_dist_normal(&seed, -50, 0));
    seed = 8;
    for (int k = 0; k < 2; ++k) show("normal", rtl_dist_normal(&seed, 3, -4));
    seed = 10;
    for (int k = 0; k < 3; ++k) show("exponential", rtl_dist_exponential(&seed, 5));
    seed = 33;
    for (int k = 0; k < 3; ++k)
        show("exponential", rtl_dist_exponential(&seed, 1000));
    seed = 9;
    show("exponential", rtl_dist_exponential(&seed, 0));
    show("exponential", rtl_dist_exponential(&seed, -3));
    seed = 10;
    for (int k = 0; k < 3; ++k) show("poisson", rtl_dist_poisson(&seed, 10));
    seed = 44;
    for (int k = 0; k < 3; ++k) show("poisson", rtl_dist_poisson(&seed, 1));
    seed = 9;
    show("poisson", rtl_dist_poisson(&seed, 0));
    seed = 10;
    for (int k = 0; k < 3; ++k) show("chi_square", rtl_dist_chi_square(&seed, 5));
    seed = 55;
    for (int k = 0; k < 3; ++k) show("chi_square", rtl_dist_chi_square(&seed, 4));
    seed = 9;
    show("chi_square", rtl_dist_chi_square(&seed, 0));
    seed = 10;
    for (int k = 0; k < 3; ++k) show("t", rtl_dist_t(&seed, 5));
    seed = 66;
    for (int k = 0; k < 3; ++k) show("t", rtl_dist_t(&seed, 1));
    seed = 9;
    show("t", rtl_dist_t(&seed, -1));
    seed = 10;
    for (int k = 0; k < 3; ++k) show("erlang", rtl_dist_erlang(&seed, 2, 10));
    seed = 77;
    for (int k = 0; k < 3; ++k) show("erlang", rtl_dist_erlang(&seed, 3, -7));
    seed = 9;
    show("erlang", rtl_dist_erlang(&seed, 0, 10));
    return 0;
}
