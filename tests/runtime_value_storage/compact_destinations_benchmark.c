#include "backend.h"
#include "llg_value.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static volatile uint64_t sink;
static void (*old_ops[])(sv4_t*, sv4_t, sv4_t) = {sv4_add_into, sv4_sub_into, sv4_mul_into};
static void (*new_ops[])(g4_t*, g4_t, g4_t) = {llg_gmp_sv4_add_into, llg_gmp_sv4_sub_into,
                                               llg_gmp_sv4_mul_into};
static int order(const void* a, const void* b) {
    double x = *(const double*)a, y = *(const double*)b;
    return x < y ? -1 : x > y;
}
static double run(int compact, unsigned op, unsigned mode, unsigned count, sv4_t a, sv4_t b, g4_t x,
                  g4_t y) {
    sv4_t left = SV4_EMPTY, right = SV4_EMPTY, dest = SV4_EMPTY;
    g4_t lhs = LLG_GMP_SV4_EMPTY, rhs = LLG_GMP_SV4_EMPTY, out = LLG_GMP_SV4_EMPTY;
    if (compact) {
        lhs = llg_gmp_sv4_clone(&x);
        rhs = llg_gmp_sv4_clone(&y);
        out = llg_gmp_sv4_zero(x.width, 0);
    } else {
        left = sv4_clone(&a);
        right = sv4_clone(&b);
        dest = sv4_zero(a.width, 0);
    }
    sv4_t* old_target = mode == 1 || mode == 3 ? &left : mode == 2 ? &right : &dest;
    g4_t* target = mode == 1 || mode == 3 ? &lhs : mode == 2 ? &rhs : &out;
    struct timespec start, finish;
    timespec_get(&start, TIME_UTC);
    for (unsigned i = 0; i < count; ++i) {
        if (compact) {
            new_ops[op](target, lhs, mode == 3 ? lhs : rhs);
            sink += llg_gmp_sv4_to_u64(*target);
        } else {
            old_ops[op](old_target, left, mode == 3 ? left : right);
            sink += sv4_to_u64(*old_target);
        }
    }
    timespec_get(&finish, TIME_UTC);
    sv4_destroy(&left);
    sv4_destroy(&right);
    sv4_destroy(&dest);
    llg_gmp_sv4_destroy(&lhs);
    llg_gmp_sv4_destroy(&rhs);
    llg_gmp_sv4_destroy(&out);
    return ((double)(finish.tv_sec - start.tv_sec) * 1e9 +
            (double)(finish.tv_nsec - start.tv_nsec)) /
           count;
}
int main(int argc, char** argv) {
    int smoke = argc == 2 && !strcmp(argv[1], "--smoke");
    if (argc > 1 && !smoke)
        return 1;
    uint32_t widths[] = {1, 64, 65, 256, 4096};
    const char* ops[] = {"add_into", "sub_into", "mul_into"};
    const char* modes[] = {"independent", "alias_left", "alias_right", "alias_both"};
    const char* states[] = {"known", "x", "z"};
    puts("backend,width,state,operation,mode,median_ns,min_ns,max_ns,payload_bytes");
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k)
        for (unsigned state = 0; state < 3; ++state) {
            uint32_t width = widths[k];
            sv4_t a = sv4_zero(width, 0), b = sv4_zero(width, 0);
            for (size_t i = 0; i < llg_sv4_words(a); ++i) {
                llg_sv4_set_word(&a, i, UINT64_C(0x731894abcdef9837), 0, 0);
                llg_sv4_set_word(&b, i, UINT64_C(0xabc731982176ab13), 0, 0);
            }
            if (state) {
                llg_sv4_set_state(&a, width - 1, state + 1);
                llg_sv4_set_state(&b, width - 1, state + 1);
            }
            g4_t x = llg_gmp_sv4_from_limbs(a.bits, a.x, a.z, width, 0),
                 y = llg_gmp_sv4_from_limbs(b.bits, b.x, b.z, width, 0);
            for (unsigned op = 0; op < 3; ++op)
                for (unsigned mode = 0; mode < 4; ++mode) {
                    unsigned count = smoke ? 16 : width == 4096 && op == 2 ? 2000 : 20000;
                    double samples[2][7];
                    for (unsigned sample = 0; sample < 7; ++sample)
                        for (unsigned step = 0; step < 2; ++step) {
                            int backend = (int)((sample + step) % 2);
                            samples[backend][sample] = run(backend, op, mode, count, a, b, x, y);
                        }
                    for (int backend = 0; backend < 2; ++backend) {
                        qsort(samples[backend], 7, sizeof(double), order);
                        printf("%s,%u,%s,%s,%s,%.2f,%.2f,%.2f,%zu\n",
                               backend ? "compact" : "legacy", width, states[state], ops[op],
                               modes[mode], samples[backend][3], samples[backend][0],
                               samples[backend][6],
                               backend ? llg_gmp_sv4_bytes(&x) : sv4_bytes(&a));
                    }
                }
            sv4_destroy(&a);
            sv4_destroy(&b);
            llg_gmp_sv4_destroy(&x);
            llg_gmp_sv4_destroy(&y);
        }
    return 0;
}
