#include "backend.h"
#include <stdio.h>
#include <stdlib.h>

#include "compact_families_compact_dispatch.h"

static size_t allocations, reallocations;
static volatile uint64_t sink;
void* __real_malloc(size_t);
void* __real_calloc(size_t, size_t);
void* __real_realloc(void*, size_t);
void* __wrap_malloc(size_t n) {
    ++allocations;
    return __real_malloc(n);
}
void* __wrap_calloc(size_t n, size_t s) {
    ++allocations;
    return __real_calloc(n, s);
}
void* __wrap_realloc(void* p, size_t n) {
    ++reallocations;
    return __real_realloc(p, n);
}
#define CHECK(c)                                                                                   \
    do {                                                                                           \
        if (!(c)) {                                                                                \
            fprintf(stderr, "family allocation check: %s\n", #c);                                  \
            abort();                                                                               \
        }                                                                                          \
    } while (0)
int main(void) {
    for (uint32_t w = 0; w <= 64; ++w)
        for (uint8_t state = 0; state < 4; ++state) {
            g4_t a = llg_gmp_sv4_fill(state, w, 1), b = llg_gmp_sv4_fill(state, w, 0);
            size_t before = allocations, re_before = reallocations;
            for (unsigned op = 0; op < 20; ++op) {
                g4_t r = compact_eval(op, a, b, a);
                sink += llg_gmp_sv4_to_u64(r);
                llg_gmp_sv4_destroy(&r);
            }
            CHECK(allocations == before && reallocations == re_before);
        }
    const uint32_t widths[] = {65, 128, 129, 4096, LLG_GMP_SUPPORTED_WIDTH_LIMIT - 1};
    for (size_t i = 0; i < sizeof(widths) / sizeof(widths[0]); ++i)
        for (uint8_t state = 0; state < 4; ++state) {
            uint32_t w = widths[i];
            g4_t a = llg_gmp_sv4_fill(state, w, 1), b = llg_gmp_sv4_clone(&a);
            size_t before = allocations, re_before = reallocations;
            for (unsigned op = 4; op < 20; ++op) {
                g4_t r = compact_eval(op, a, b, a);
                sink += llg_gmp_sv4_to_u64(r);
                llg_gmp_sv4_destroy(&r);
            }
            CHECK(allocations == before && reallocations == re_before);
            g4_t zero = llg_gmp_sv4_zero(8, 0);
            before = allocations;
            g4_t r = llg_gmp_sv4_shl(a, zero);
            CHECK(allocations == before + 1);
            CHECK(r.data.wide.a != a.data.wide.a);
            llg_gmp_sv4_destroy(&r);
            llg_gmp_sv4_destroy(&a);
            llg_gmp_sv4_destroy(&b);
            a = llg_gmp_sv4_zero(w, 0);
            llg_gmp_sv4_set_state(&a, w - 1, 2);
            g4_t one = llg_gmp_sv4_from_u64(1, 8, 0);
            before = allocations;
            re_before = reallocations;
            r = llg_gmp_sv4_shl(a, one);
            CHECK(allocations == before + 1 && reallocations == re_before + 1);
            CHECK(!r.data.wide.b && llg_gmp_sv4_bytes(&r) == 8u * llg_gmp_sv4_words(r));
            llg_gmp_sv4_destroy(&r);
            llg_gmp_sv4_destroy(&a);
        }
    puts("families: allocation-free inline operations and wide predicates; independent shifts and "
         "B removal");
    return 0;
}
