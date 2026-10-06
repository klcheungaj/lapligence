/* Native value-operation witness for perf/scripts/value_backends.py.
 *
 * Compiled once per generated project against that project's selected value
 * facade and runtime archive, so legacy, compact/portable and compact/GMP are
 * measured through the same public names. `layout` prints the selected
 * descriptor size/alignment and the requested payload bytes of known and X
 * owners; `timing` prints process-CPU ns per operation for each ownership mode.
 *
 * Ownership modes compare like with like on every backend:
 *   fresh   - sv4_replace(&r, sv4_op(a, b)): a returned independent owner.
 *   to_new  - destroy r, then sv4_op_to(&r, &a, &b): the generated-statement
 *             pattern, where every temporary starts empty.
 *   to_live - sv4_op_to(&r, &a, &b) into a live owner of the result width.
 * The 65-bit multiplication rows are the required fresh-owner regression
 * witness from the GMP prototype. */
#define _POSIX_C_SOURCE 200809L

#include "llg_value.h"
#include "value/destinations.h"

#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

enum { OP_ADD, OP_MUL, OP_XOR, OP_COUNT };
enum { MODE_FRESH, MODE_TO_NEW, MODE_TO_LIVE, MODE_COUNT };

static const char* const op_names[OP_COUNT] = {"add", "mul", "xor"};
static const char* const mode_names[MODE_COUNT] = {"fresh", "to_new", "to_live"};
static const uint32_t widths[] = {32, 64, 65, 128, 256, 1024, 4096};

static volatile uint64_t sink;
static uint64_t seed = UINT64_C(0x439fe63762de2301);

static uint64_t next_word(void) {
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    return seed;
}

static double cpu_seconds(void) {
    struct timespec now;
    if (clock_gettime(CLOCK_PROCESS_CPUTIME_ID, &now) != 0) {
        perror("clock_gettime");
        exit(2);
    }
    return (double)now.tv_sec + (double)now.tv_nsec * 1e-9;
}

static sv4_t random_value(uint32_t width, int unknown) {
    size_t words = ((size_t)width + 63u) / 64u;
    uint64_t* planes = calloc(words * 3u, sizeof(uint64_t));
    if (!planes) {
        fputs("allocation failed\n", stderr);
        exit(2);
    }
    for (size_t i = 0; i < words; ++i) {
        planes[i] = next_word();
        if (unknown && i == 0) planes[words + i] = UINT64_C(1) << 3;
    }
    sv4_t value = sv4_from_limbs(planes, planes + words, planes + 2u * words, width, 0);
    free(planes);
    return value;
}

static void apply(int op, int mode, sv4_t* r, const sv4_t* a, const sv4_t* b) {
    if (mode == MODE_FRESH) {
        switch (op) {
        case OP_ADD: sv4_replace(r, sv4_add(*a, *b)); break;
        case OP_MUL: sv4_replace(r, sv4_mul(*a, *b)); break;
        default: sv4_replace(r, sv4_xor(*a, *b)); break;
        }
        return;
    }
    if (mode == MODE_TO_NEW) sv4_destroy(r);
    switch (op) {
    case OP_ADD: sv4_add_to(r, a, b); break;
    case OP_MUL: sv4_mul_to(r, a, b); break;
    default: sv4_xor_to(r, a, b); break;
    }
}

static double measure(int op, int mode, size_t count, const sv4_t* a, const sv4_t* b) {
    sv4_t r = SV4_EMPTY;
    apply(op, MODE_FRESH, &r, a, b);
    double start = cpu_seconds();
    for (size_t i = 0; i < count; ++i) apply(op, mode, &r, a, b);
    double elapsed = cpu_seconds() - start;
    sink ^= (uint64_t)llg_sv4_width(r) ^ (uint64_t)sv4_is_unknown(r);
    sv4_destroy(&r);
    return elapsed;
}

static int layout(void) {
    printf("layout\tdescriptor_bytes\t%zu\n", sizeof(sv4_t));
    printf("layout\tdescriptor_align\t%zu\n", (size_t)_Alignof(sv4_t));
    for (size_t i = 0; i < sizeof(widths) / sizeof(widths[0]); ++i) {
        sv4_t known = sv4_zero(widths[i], 0);
        sv4_t unknown = sv4_x(widths[i], 0);
        printf("payload\t%" PRIu32 "\t%zu\t%zu\n", widths[i], sv4_bytes(&known),
               sv4_bytes(&unknown));
        sv4_destroy(&known);
        sv4_destroy(&unknown);
    }
    return 0;
}

static int timing(double target_seconds, int repeats) {
    for (size_t w = 0; w < sizeof(widths) / sizeof(widths[0]); ++w) {
        for (int unknown = 0; unknown <= 1; ++unknown) {
            sv4_t a = random_value(widths[w], unknown);
            sv4_t b = random_value(widths[w], 0);
            for (int op = 0; op < OP_COUNT; ++op) {
                if (op == OP_MUL && widths[w] > 1024) continue;
                for (int mode = 0; mode < MODE_COUNT; ++mode) {
                    size_t count = 16;
                    while (measure(op, mode, count, &a, &b) < target_seconds &&
                           count < ((size_t)1 << 28))
                        count *= 2;
                    printf("timing\t%" PRIu32 "\t%s\t%s\t%s\t%zu", widths[w],
                           unknown ? "xz" : "known", op_names[op], mode_names[mode],
                           count);
                    for (int r = 0; r < repeats; ++r)
                        printf("\t%.3f", measure(op, mode, count, &a, &b) * 1e9 / (double)count);
                    putchar('\n');
                }
            }
            sv4_destroy(&a);
            sv4_destroy(&b);
        }
    }
    fprintf(stderr, "sink %" PRIu64 "\n", (uint64_t)sink);
    return 0;
}

int main(int argc, char** argv) {
    llg_value_require_abi();
    if (argc >= 2 && strcmp(argv[1], "layout") == 0) return layout();
    if (argc >= 2 && strcmp(argv[1], "timing") == 0) {
        double target = argc >= 3 ? atof(argv[2]) : 0.005;
        int repeats = argc >= 4 ? atoi(argv[3]) : 3;
        if (target <= 0.0 || repeats <= 0) return 2;
        return timing(target, repeats);
    }
    fputs("usage: value_ops_bench layout | timing [seconds] [repeats]\n", stderr);
    return 2;
}
