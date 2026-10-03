#define _POSIX_C_SOURCE 200809L
#include "backend.h"
#include "llg_value.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "compact_families_dispatch.h"
#include "compact_families_compact_dispatch.h"

static volatile uint64_t sink;
static int order(const void* a, const void* b) {
    double x = *(const double*)a, y = *(const double*)b;
    return x < y ? -1 : x > y;
}
static struct timespec now(void) {
    struct timespec t;
    timespec_get(&t, TIME_UTC);
    return t;
}
static double run(int compact, unsigned op, unsigned count, sv4_t a, sv4_t b, sv4_t c, g4_t x,
                  g4_t y, g4_t z) {
    uint64_t old_word = a.bits[0], old_b = b.bits[0];
    uint64_t* xa = x.width <= 64 ? &x.data.small.a : x.data.wide.a;
    uint64_t* ya = y.width <= 64 ? &y.data.small.a : y.data.wide.a;
    uint64_t initial = *xa, initial_y = *ya;
    uint64_t unknown = x.width <= 64 ? x.data.small.b : x.data.wide.b ? x.data.wide.b[0] : 0;
    uint64_t care = ~unknown & LLG_GMP_MASK(x.width);
    struct timespec begin = now();
    for (unsigned i = 0; i < count; ++i) {
        uint64_t variation = (uint64_t)i & care;
        if (compact) {
            *xa = initial ^ variation;
            if (op >= 13 && op <= 16)
                *ya = initial_y ^ variation;
            g4_t r = compact_eval(op, x, y, z);
            sink += llg_gmp_sv4_to_u64(r);
            llg_gmp_sv4_destroy(&r);
        } else {
            a.bits[0] = old_word ^ variation;
            if (op >= 13 && op <= 16)
                b.bits[0] = old_b ^ variation;
            sv4_t r = evaluate(op, a, b, c);
            sink += sv4_to_u64(r);
            sv4_destroy(&r);
        }
    }
    struct timespec finish = now();
    a.bits[0] = old_word;
    b.bits[0] = old_b;
    *xa = initial;
    *ya = initial_y;
    double elapsed =
        (double)(finish.tv_sec - begin.tv_sec) * 1e9 + (double)(finish.tv_nsec - begin.tv_nsec);
    return elapsed / count;
}
int main(int argc, char** argv) {
    int smoke = argc == 2 && !strcmp(argv[1], "--smoke");
    if (argc > 1 && !smoke)
        return 1;
    const uint32_t widths[] = {1, 8, 32, 64, 65, 128, 256, 1024, 4096};
    const char* names[] = {"shl",         "shr",       "ashl",       "ashr",       "reduce_and",
                           "reduce_nand", "reduce_or", "reduce_nor", "reduce_xor", "reduce_xnor",
                           "countones",   "onehot",    "onehot0",    "casex_eq",   "casez_eq",
                           "wild_eq",     "wild_neq",  "logimpl",    "logequiv",   "inside_range"};
    puts("backend,width,state,operation,mode,median_ns,min_ns,max_ns,payload_bytes,descriptor_"
         "bytes");
    for (size_t wi = 0; wi < sizeof(widths) / sizeof(widths[0]); ++wi)
        for (unsigned state = 0; state < 2; ++state) {
            uint32_t w = widths[wi];
            size_t n = (w + 63u) / 64u;
            uint64_t* p = (uint64_t*)calloc(3 * n, sizeof(uint64_t));
            if (!p)
                return 1;
            for (size_t i = 0; i < n; ++i) {
                p[i] = UINT64_C(0xaaaaaaaaaaaaaaaa);
                p[n + i] = state ? UINT64_C(0x0000001800008101) : 0;
                p[2 * n + i] = state ? UINT64_C(0x0000080000000000) : 0;
                p[i] &= ~(p[n + i] | p[2 * n + i]);
            }
            sv4_t a = sv4_from_limbs(p, p + n, p + 2 * n, w, 1), same = sv4_clone(&a),
                  shift = sv4_from_u64(3, 8, 0), low = sv4_zero(w, 1), high = sv4_fill(1, w, 1);
            g4_t x = llg_gmp_sv4_from_limbs(p, p + n, p + 2 * n, w, 1),
                 equal = llg_gmp_sv4_clone(&x), count = llg_gmp_sv4_from_u64(3, 8, 0),
                 lo = llg_gmp_sv4_zero(w, 1), hi = llg_gmp_sv4_fill(1, w, 1);
            llg_sv4_set_state(&low, w - 1u, 1);
            llg_sv4_set_state(&high, w - 1u, 0);
            llg_gmp_sv4_set_state(&lo, w - 1u, 1);
            llg_gmp_sv4_set_state(&hi, w - 1u, 0);
            for (unsigned op = 0; op < 20; ++op) {
                sv4_t b = op < 4 ? shift : op == 19 ? low : same;
                g4_t y = op < 4 ? count : op == 19 ? lo : equal;
                unsigned repetitions = smoke ? 16 : w >= 1024 ? 1000 : 10000;
                double samples[2][7];
                for (unsigned sample = 0; sample < 7; ++sample)
                    for (unsigned step = 0; step < 2; ++step) {
                        int backend = (int)((sample + step) % 2);
                        samples[backend][sample] =
                            run(backend, op, repetitions, a, b, high, x, y, hi);
                    }
                for (unsigned backend = 0; backend < 2; ++backend) {
                    qsort(samples[backend], 7, sizeof(double), order);
                    printf("%s,%u,%s,%s,fresh,%.2f,%.2f,%.2f,%zu,%zu\n",
                           backend ? "compact" : "legacy", w, state ? "xz" : "known", names[op],
                           samples[backend][3], samples[backend][0], samples[backend][6],
                           backend ? llg_gmp_sv4_bytes(&x) : sv4_bytes(&a),
                           backend ? sizeof(g4_t) : sizeof(sv4_t));
                }
            }
            sv4_destroy(&a);
            sv4_destroy(&same);
            sv4_destroy(&shift);
            sv4_destroy(&low);
            sv4_destroy(&high);
            llg_gmp_sv4_destroy(&x);
            llg_gmp_sv4_destroy(&equal);
            llg_gmp_sv4_destroy(&count);
            llg_gmp_sv4_destroy(&lo);
            llg_gmp_sv4_destroy(&hi);
            free(p);
        }
    return 0;
}
