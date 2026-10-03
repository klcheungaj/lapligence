#include "llg_value.h"
#include "backend.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
static volatile uint64_t sink;
static double elapsed(struct timespec a, struct timespec b) {
    return (double)(b.tv_sec - a.tv_sec) * 1e9 + (double)(b.tv_nsec - a.tv_nsec);
}
static int order(const void* a, const void* b) {
    double x = *(const double*)a, y = *(const double*)b;
    return x < y ? -1 : x > y;
}
static double run(int compact, unsigned op, unsigned count, sv4_t old, g4_t value) {
    uint32_t w = value.width;
    uint64_t initial = llg_gmp_sv4_vpi_word(value, 0).aval;
    uint64_t unknown = llg_gmp_sv4_vpi_word(value, 0).bval;
    uint64_t known = ~unknown & LLG_GMP_MASK(w);
    llg_gmp_sv4_select_plan_t plan = llg_gmp_sv4_select_plan_init(w);
    sv4_select_plan_t old_plan = sv4_select_plan_init(w);
    g4_t def = llg_gmp_sv4_x(1, 0);
    sv4_t old_def = sv4_x(1, 0);
    struct timespec start, end;
    timespec_get(&start, TIME_UTC);
    for (unsigned i = 0; i < count; ++i) {
        uint64_t varied = initial ^ ((uint64_t)i & known);
        if (compact) {
            if (w <= 64)
                value.data.small.a = varied;
            else
                value.data.wide.a[0] = varied;
            g4_t result;
            switch (op) {
            case 0:
                result = llg_gmp_sv4_bit_select(value, w / 2);
                break;
            case 1:
                result = llg_gmp_sv4_part_select(value, w - 1, w / 3);
                break;
            case 2:
                result = llg_gmp_sv4_idx_part_select(value, 0, w, 0);
                break;
            case 3:
                result = llg_gmp_sv4_select_plan_read(value, &plan);
                break;
            case 4:
                result = llg_gmp_sv4_concat(value, value);
                break;
            case 5:
                result = llg_gmp_sv4_repeat(value, 3);
                break;
            case 6:
                result = llg_gmp_sv4_stream(value, 7, 1);
                break;
            case 7:
                result = llg_gmp_sv4_unstream(value, 7, 1);
                break;
            case 8:
                result = llg_gmp_sv4_array_conditional_merge(value, value, def);
                break;
            case 9:
                result = llg_gmp_sv4_repeat_count(value);
                break;
            case 10:
                result = llg_gmp_sv4_clone(&value);
                llg_gmp_sv4_idx_part_select_set(&result, 1, w, 0, value);
                break;
            case 11:
                result = llg_gmp_sv4_clone(&value);
                llg_gmp_sv4_select_plan_set(&result, &plan, value);
                break;
            default:
                result = llg_gmp_sv4_part_select(value, 0, w - 1);
                break;
            }
            sink += llg_gmp_sv4_to_u64(result);
            llg_gmp_sv4_destroy(&result);
        } else {
            old.bits[0] = varied & ~unknown;
            sv4_t result;
            switch (op) {
            case 0:
                result = sv4_bit_select(old, w / 2);
                break;
            case 1:
                result = sv4_part_select(old, w - 1, w / 3);
                break;
            case 2:
                result = sv4_idx_part_select(old, 0, w, 0);
                break;
            case 3:
                result = sv4_select_plan_read(old, &old_plan);
                break;
            case 4:
                result = sv4_concat(old, old);
                break;
            case 5:
                result = sv4_repeat(old, 3);
                break;
            case 6:
                result = sv4_stream(old, 7, 1);
                break;
            case 7:
                result = sv4_unstream(old, 7, 1);
                break;
            case 8:
                result = sv4_array_conditional_merge(old, old, old_def);
                break;
            case 9:
                result = sv4_repeat_count(old);
                break;
            case 10:
                result = sv4_clone(&old);
                sv4_idx_part_select_set(&result, 1, w, 0, old);
                break;
            case 11:
                result = sv4_clone(&old);
                sv4_select_plan_set(&result, &old_plan, old);
                break;
            default:
                result = sv4_part_select(old, 0, w - 1);
                break;
            }
            sink += sv4_to_u64(result);
            sv4_destroy(&result);
        }
    }
    timespec_get(&end, TIME_UTC);
    old.bits[0] = initial & ~unknown;
    if (w > 64)
        value.data.wide.a[0] = initial;
    llg_gmp_sv4_destroy(&def);
    sv4_destroy(&old_def);
    return elapsed(start, end) / count;
}
int main(int argc, char** argv) {
    int smoke = argc > 1 && !strcmp(argv[1], "--smoke");
    const char* names[] = {"bit",
                           "part",
                           "indexed",
                           "plan-read",
                           "concat",
                           "repeat",
                           "stream",
                           "unstream",
                           "array",
                           "repeat-count",
                           "indexed-write-clone",
                           "plan-write-clone",
                           "reversed-part"};
    uint32_t widths[] = {1, 64, 65, 256, 4096};
    puts("width,unknown,operation,legacy_ns,compact_ns,legacy_min,legacy_max,compact_min,compact_"
         "max");
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k)
        for (int unknown = 0; unknown < 2; ++unknown) {
            uint32_t w = widths[k];
            g4_t v = llg_gmp_sv4_zero(w, 0);
            sv4_t old = sv4_zero(w, 0);
            for (size_t j = 0; j < llg_gmp_sv4_words(v); ++j) {
                uint64_t bits = UINT64_C(0x5555555555555555),
                         x = unknown ? UINT64_C(0x0002000200020002) : 0,
                         z = unknown ? UINT64_C(0x0080008000800080) : 0;
                if (w == 1 && unknown) {
                    bits = 0;
                    x = 1;
                }
                bits &= ~(x | z);
                llg_gmp_sv4_set_word(&v, j, bits, x, z);
                llg_sv4_set_word(&old, j, bits, x, z);
            }
            for (unsigned op = 0; op < sizeof(names) / sizeof(names[0]); ++op) {
                double a[7], b[7];
                unsigned count = smoke ? 10 : w > 64 ? 10000 : 100000;
                for (unsigned sample = 0; sample < 7; ++sample) {
                    if (sample % 2) {
                        b[sample] = run(1, op, count, old, v);
                        a[sample] = run(0, op, count, old, v);
                    } else {
                        a[sample] = run(0, op, count, old, v);
                        b[sample] = run(1, op, count, old, v);
                    }
                }
                qsort(a, 7, sizeof(double), order);
                qsort(b, 7, sizeof(double), order);
                printf("%u,%d,%s,%.2f,%.2f,%.2f,%.2f,%.2f,%.2f\n", w, unknown, names[op], a[3],
                       b[3], a[0], a[6], b[0], b[6]);
            }
            llg_gmp_sv4_destroy(&v);
            sv4_destroy(&old);
        }
    return 0;
}
