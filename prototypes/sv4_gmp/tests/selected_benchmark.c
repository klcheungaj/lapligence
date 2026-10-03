#include "sv4.h"
#include "check.h"
#include <inttypes.h>
#include <time.h>

static volatile uint64_t sink;
static uint64_t seed = UINT64_C(0x439fe63762de2301);
enum { ADD, MUL, AND, CLONE, COPY };
static const char *names[] = {"add_known", "mul_known", "and", "clone_known", "copy_known"};

static uint64_t next_word(void) {
    seed ^= seed << 13;
    seed ^= seed >> 7;
    seed ^= seed << 17;
    return seed;
}

static double measure(int operation, size_t count, sv4_t a, sv4_t b) {
    sv4_t result = SV4_EMPTY;
    clock_t start = clock();
    CHECK(start != (clock_t)-1);
    for (size_t i = 0; i < count; ++i) {
        switch (operation) {
        case ADD: sv4_replace(&result, sv4_add(a, b)); break;
        case MUL: sv4_replace(&result, sv4_mul(a, b)); break;
        case AND: sv4_replace(&result, sv4_and(a, b)); break;
        case CLONE: sv4_replace(&result, sv4_clone(&a)); break;
        default: sv4_copy(&result, &a); break;
        }
    }
    clock_t end = clock();
    CHECK(end != (clock_t)-1);
    sink ^= sv4_to_u64(result);
    sv4_destroy(&result);
    return (double)(end - start) / CLOCKS_PER_SEC;
}

static void run(uint32_t width, int unknown, int operation) {
    size_t words = ((size_t)width + 63u) / 64u;
    uint64_t *planes = calloc(words * 6u, sizeof(uint64_t));
    CHECK(planes);
    uint64_t *bits = planes, *x = planes + words, *z = planes + 2u * words;
    uint64_t *bits2 = planes + 3u * words, *x2 = planes + 4u * words, *z2 = planes + 5u * words;
    for (size_t i = 0; i < words; ++i) {
        if (unknown) {
            x[i] = next_word(); z[i] = next_word() & ~x[i];
            x2[i] = next_word(); z2[i] = next_word() & ~x2[i];
        }
        bits[i] = next_word() & ~(x[i] | z[i]);
        bits2[i] = next_word() & ~(x2[i] | z2[i]);
    }
    sv4_t a = sv4_from_limbs(bits, x, z, width, 0);
    sv4_t b = sv4_from_limbs(bits2, x2, z2, width, 0);
    size_t count = 16;
    while (measure(operation, count, a, b) < 0.005 && count < (1u << 24)) count *= 2;
    printf("{\"kind\":\"timing\",\"width\":%u,\"operation\":\"%s%s\",\"iterations\":%zu,\"ns\":[",
           width, names[operation], operation == AND ? (unknown ? "_xz" : "_known") : "", count);
    for (size_t repeat = 0; repeat < 5; ++repeat)
        printf("%s%.6f", repeat ? "," : "", measure(operation, count, a, b) * 1e9 / count);
    puts("]}");
    sv4_destroy(&b);
    sv4_destroy(&a);
    free(planes);
}

int main(void) {
    printf("{\"kind\":\"layout\",\"backend\":\"%s\",\"descriptor\":%zu,\"clock_ticks_per_second\":%ld}\n",
           LLG_SV4_BACKEND_NAME, sizeof(sv4_t), (long)CLOCKS_PER_SEC);
    static const uint32_t widths[] = {1, 8, 32, 64, 65, 256, 1024, 4096, 16384};
    for (size_t i = 0; i < sizeof(widths) / sizeof(widths[0]); ++i) {
        sv4_t known = sv4_zero(widths[i], 0), xz = sv4_x(widths[i], 0);
        printf("{\"kind\":\"memory\",\"width\":%u,\"known_payload\":%zu,\"xz_payload\":%zu}\n",
               widths[i], sv4_bytes(&known), sv4_bytes(&xz));
        sv4_destroy(&xz);
        sv4_destroy(&known);
        for (int operation = ADD; operation <= COPY; ++operation) run(widths[i], 0, operation);
        run(widths[i], 1, AND);
    }
    fprintf(stderr, "benchmark checksum: %" PRIu64 "\n", sink);
    return 0;
}
