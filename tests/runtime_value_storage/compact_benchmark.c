#define _POSIX_C_SOURCE 200809L
#include "llg_value.h"
#include "backend.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static volatile uint64_t sink;
static struct timespec now(void) {
    struct timespec t;
    timespec_get(&t, TIME_UTC);
    return t;
}
static int order(const void* a, const void* b) {
    double x = *(const double*)a, y = *(const double*)b;
    return x < y ? -1 : x > y;
}
static double run(int compact, unsigned op, int reuse, unsigned count, sv4_t a, sv4_t b, g4_t x,
                  g4_t y) {
    sv4_t old = SV4_EMPTY;
    g4_t result = LLG_GMP_SV4_EMPTY;
    if (op == 2) {
        b = sv4_clone(&a);
        y = llg_gmp_sv4_clone(&x);
    }
    uint64_t old_word = a.bits[0];
    uint64_t new_word = x.width <= 64 ? x.data.small.a : x.data.wide.a[0];
    uint64_t unknown = x.width <= 64 ? x.data.small.b : x.data.wide.b ? x.data.wide.b[0] : 0;
    uint64_t known_mask = ~unknown & LLG_GMP_MASK(x.width);
    struct timespec start = now();
    for (unsigned i = 0; i < count; ++i) {
        if (compact) {
            uint64_t varied = new_word ^ ((uint64_t)i & known_mask);
            if (x.width <= 64)
                x.data.small.a = varied;
            else
                x.data.wide.a[0] = varied;
            if (op == 2) {
                if (y.width <= 64)
                    y.data.small.a = varied;
                else
                    y.data.wide.a[0] = varied;
            }
            if (op == 3) {
                if (reuse)
                    llg_gmp_sv4_copy(&result, &x);
                else
                    result = llg_gmp_sv4_clone(&x);
            } else {
                g4_t r = op == 0   ? llg_gmp_sv4_add(x, y)
                         : op == 1 ? llg_gmp_sv4_and(x, y)
                         : op == 2 ? llg_gmp_sv4_eq(x, y)
                         : op == 4 ? llg_gmp_sv4_mul(x, y)
                                   : llg_gmp_sv4_div(x, y);
                if (reuse)
                    llg_gmp_sv4_replace(&result, r);
                else
                    result = r;
            }
            sink += llg_gmp_sv4_to_u64(result);
            if (!reuse)
                llg_gmp_sv4_destroy(&result);
        } else {
            a.bits[0] = old_word ^ ((uint64_t)i & known_mask);
            if (op == 2)
                b.bits[0] = a.bits[0];
            if (op == 3) {
                if (reuse)
                    sv4_copy(&old, &a);
                else
                    old = sv4_clone(&a);
            } else {
                sv4_t r = op == 0   ? sv4_add(a, b)
                          : op == 1 ? sv4_and(a, b)
                          : op == 2 ? sv4_eq(a, b)
                          : op == 4 ? sv4_mul(a, b)
                                    : sv4_div(a, b);
                if (reuse)
                    sv4_replace(&old, r);
                else
                    old = r;
            }
            sink += sv4_to_u64(old);
            if (!reuse)
                sv4_destroy(&old);
        }
    }
    struct timespec finish = now();
    double elapsed =
        (double)(finish.tv_sec - start.tv_sec) * 1e9 + (double)(finish.tv_nsec - start.tv_nsec);
    a.bits[0] = old_word;
    if (x.width > 64)
        x.data.wide.a[0] = new_word;
    if (op == 2) {
        sv4_destroy(&b);
        llg_gmp_sv4_destroy(&y);
    }
    sv4_destroy(&old);
    llg_gmp_sv4_destroy(&result);
    return elapsed / count;
}
int main(int argc, char** argv) {
    int smoke = argc == 2 && !strcmp(argv[1], "--smoke");
    if (argc > 1 && !smoke)
        return 1;
    unsigned widths[] = {1, 8, 32, 64, 65, 128, 256, 1024, 4096};
    const char* ops[] = {"add", "and", "eq", "copy", "mul", "div"};
    puts("backend,width,state,operation,mode,median_ns,min_ns,max_ns,payload_bytes,descriptor_"
         "bytes");
    for (size_t i = 0; i < sizeof(widths) / sizeof(widths[0]); ++i)
        for (int unknown = 0; unknown < 2; ++unknown) {
            uint32_t w = widths[i];
            size_t n = (w + 63u) / 64u;
            uint64_t* p = (uint64_t*)calloc(3 * n, sizeof(uint64_t));
            if (!p)
                return 1;
            for (size_t j = 0; j < n; ++j) {
                p[j] = UINT64_C(0x731894abcdef9837);
                if (unknown) {
                    p[n + j] = UINT64_C(0x0000001800008100);
                    p[2 * n + j] = UINT64_C(0x0000080000000000);
                    p[j] &= ~(p[n + j] | p[2 * n + j]);
                }
            }
            if (unknown) {
                p[n] |= 1;
                p[0] &= ~UINT64_C(1);
            }
            sv4_t a = sv4_from_limbs(p, p + n, p + 2 * n, w, 0), b = sv4_from_u64(3, w, 0);
            g4_t x = llg_gmp_sv4_from_limbs(p, p + n, p + 2 * n, w, 0),
                 y = llg_gmp_sv4_from_u64(3, w, 0);
            for (unsigned op = 0; op < 6; ++op)
                for (int reuse = 0; reuse < 2; ++reuse) {
                    unsigned count = smoke ? 32 : w >= 1024 && op >= 4 ? 1000 : 20000;
                    double samples[2][7];
                    for (unsigned k = 0; k < 7; ++k)
                        for (unsigned step = 0; step < 2; ++step) {
                            int backend = (int)((k + step) % 2);
                            samples[backend][k] = run(backend, op, reuse, count, a, b, x, y);
                        }
                    for (int backend = 0; backend < 2; ++backend) {
                        qsort(samples[backend], 7, sizeof(double), order);
                        printf("%s,%u,%s,%s,%s,%.2f,%.2f,%.2f,%zu,%zu\n",
                               backend ? "compact" : "legacy", w, unknown ? "xz" : "known", ops[op],
                               reuse ? "reuse" : "fresh", samples[backend][3], samples[backend][0],
                               samples[backend][6], backend ? llg_gmp_sv4_bytes(&x) : sv4_bytes(&a),
                               backend ? sizeof(g4_t) : sizeof(sv4_t));
                    }
                }
            sv4_destroy(&a);
            sv4_destroy(&b);
            llg_gmp_sv4_destroy(&x);
            llg_gmp_sv4_destroy(&y);
            free(p);
        }
    return 0;
}
