#include "backend.h"
#include <stdio.h>
#include <stdlib.h>

static size_t allocations, reallocations;
static volatile uint64_t sink;
void* __real_malloc(size_t);
void* __real_calloc(size_t, size_t);
void* __real_realloc(void*, size_t);
void* __wrap_malloc(size_t n) {
    ++allocations;
    return __real_malloc(n);
}
void* __wrap_calloc(size_t n, size_t size) {
    ++allocations;
    return __real_calloc(n, size);
}
void* __wrap_realloc(void* p, size_t n) {
    ++reallocations;
    return __real_realloc(p, n);
}
#define CHECK(c)                                                                                   \
    do {                                                                                           \
        if (!(c)) {                                                                                \
            fprintf(stderr, "allocation check failed: %s\n", #c);                                  \
            abort();                                                                               \
        }                                                                                          \
    } while (0)
int main(void) {
    for (uint32_t w = 0; w <= 64; ++w) {
        size_t before = allocations, r_before = reallocations;
        g4_t a = llg_gmp_sv4_from_u64(23, w, 1), b = llg_gmp_sv4_from_i64(-3, w),
             x = llg_gmp_sv4_x(w, 0);
        g4_t (*binary[])(g4_t, g4_t) = {
            llg_gmp_sv4_add,     llg_gmp_sv4_sub,      llg_gmp_sv4_mul,    llg_gmp_sv4_div,
            llg_gmp_sv4_mod,     llg_gmp_sv4_pow,      llg_gmp_sv4_and,    llg_gmp_sv4_or,
            llg_gmp_sv4_xor,     llg_gmp_sv4_xnor,     llg_gmp_sv4_eq,     llg_gmp_sv4_neq,
            llg_gmp_sv4_case_eq, llg_gmp_sv4_case_neq, llg_gmp_sv4_lt,     llg_gmp_sv4_le,
            llg_gmp_sv4_gt,      llg_gmp_sv4_ge,       llg_gmp_sv4_logand, llg_gmp_sv4_logor};
        for (size_t i = 0; i < sizeof(binary) / sizeof(binary[0]); ++i) {
            g4_t r = binary[i](a, b);
            sink += llg_gmp_sv4_to_u64(r);
            llg_gmp_sv4_destroy(&r);
            r = binary[i](a, x);
            sink += llg_gmp_sv4_to_u64(r);
            llg_gmp_sv4_destroy(&r);
        }
        g4_t r = llg_gmp_sv4_mux(x, a, b);
        llg_gmp_sv4_destroy(&r);
        r = llg_gmp_sv4_clone(&a);
        llg_gmp_sv4_copy(&r, &b);
        llg_gmp_sv4_assign(&r, a);
        llg_gmp_sv4_set_state(&r, 0, 2);
        llg_gmp_sv4_set_word(&r, 0, 42, 1, 2);
        llg_gmp_sv4_word_t out[2];
        llg_gmp_sv4_export_words(r, 0, out, 2);
        llg_gmp_sv4_import_words(&r, 0, out, 2);
        llg_gmp_sv4_vpi_word_t ab[2];
        llg_gmp_sv4_export_vpi_words(r, 0, ab, 2);
        llg_gmp_sv4_import_vpi_words(&r, 0, ab, 2);
        llg_gmp_sv4_destroy(&r);
        llg_gmp_sv4_destroy(&a);
        llg_gmp_sv4_destroy(&b);
        llg_gmp_sv4_destroy(&x);
        CHECK(allocations == before && reallocations == r_before);
    }
    g4_t a = llg_gmp_sv4_from_u64(23, 65, 0), b = llg_gmp_sv4_from_u64(3, 65, 0),
         c = llg_gmp_sv4_from_u64(3, 8, 0);
    size_t before = allocations, r_before = reallocations;
    g4_t r = llg_gmp_sv4_mul(a, b);
    CHECK(allocations == before + 1 && reallocations == r_before);
    llg_gmp_sv4_destroy(&r);
    before = allocations;
    r = llg_gmp_sv4_add(a, c);
    CHECK(allocations == before + 1 && reallocations == r_before);
    llg_gmp_sv4_destroy(&r);
    before = allocations;
    r = llg_gmp_sv4_sub(c, a);
    CHECK(allocations == before + 1 && reallocations == r_before);
    llg_gmp_sv4_destroy(&r);
    llg_gmp_sv4_destroy(&a);
    llg_gmp_sv4_destroy(&b);
    llg_gmp_sv4_destroy(&c);
    uint32_t boundary_widths[] = {8128, 8129, 8192, 8193};
    for (size_t k = 0; k < sizeof(boundary_widths) / sizeof(boundary_widths[0]); ++k) {
        g4_t ones = llg_gmp_sv4_fill(1, boundary_widths[k], 0);
        g4_t product = llg_gmp_sv4_mul(ones, ones);
        CHECK(!llg_gmp_sv4_is_unknown(product));
        for (size_t i = 0; i < llg_gmp_sv4_words(product); ++i)
            CHECK(llg_gmp_sv4_word(product, i, 0) == (i == 0 ? 1u : 0u));
        llg_gmp_sv4_destroy(&ones);
        llg_gmp_sv4_destroy(&product);
    }
    puts("small paths allocate nothing; 65-bit mul/mixed add/sub allocate result only");
    return 0;
}
