#include "backend.h"
#include <stdio.h>
#include <stdlib.h>
#define CHECK(c)                                                                                   \
    do {                                                                                           \
        if (!(c)) {                                                                                \
            fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, #c);                                \
            abort();                                                                               \
        }                                                                                          \
    } while (0)
void* __real_malloc(size_t);
void* __real_calloc(size_t, size_t);
void* __real_realloc(void*, size_t);
static size_t calls;
void* __wrap_malloc(size_t size) {
    ++calls;
    return __real_malloc(size);
}
void* __wrap_calloc(size_t count, size_t size) {
    ++calls;
    return __real_calloc(count, size);
}
void* __wrap_realloc(void* ptr, size_t size) {
    ++calls;
    return __real_realloc(ptr, size);
}
int main(void) {
    g4_t dst = llg_gmp_sv4_zero(256, 0), source = llg_gmp_sv4_zero(256, 0);
    llg_gmp_sv4_set_state(&source, 255, 2);
    size_t before = calls;
    llg_gmp_sv4_idx_part_select_set(&dst, 32, 65, 0, source);
    CHECK(calls == before && llg_gmp_sv4_bytes(&dst) == 32);
    llg_gmp_sv4_select_plan_t plan = {256, 256, 5, 3, 130};
    llg_gmp_sv4_select_plan_set(&dst, &plan, source);
    CHECK(calls == before && llg_gmp_sv4_bytes(&dst) == 32);
    llg_gmp_sv4_bit_select_set(&dst, UINT64_MAX, source);
    CHECK(calls == before);
    g4_t unknown = llg_gmp_sv4_fill(3, 7, 0);
    llg_gmp_sv4_idx_part_select_set(&dst, 70, 7, 0, unknown);
    CHECK(calls == before + 1 && llg_gmp_sv4_bytes(&dst) == 64);
    g4_t known = llg_gmp_sv4_zero(7, 0);
    llg_gmp_sv4_idx_part_select_set(&dst, 70, 7, 0, known);
    CHECK(calls == before + 2 && llg_gmp_sv4_bytes(&dst) == 32);
    before = calls;
    g4_t a = llg_gmp_sv4_from_masks(9, 2, 4, 8, 0);
    g4_t r = llg_gmp_sv4_repeat(a, 4), stream = llg_gmp_sv4_stream(r, 3, 1);
    g4_t cat = llg_gmp_sv4_concat(a, a), part = llg_gmp_sv4_part_select(stream, 29, 1);
    g4_t merge = llg_gmp_sv4_array_conditional_merge(cat, cat, a);
    CHECK(calls == before);
    llg_gmp_sv4_part_select_set(&part, 23, 1, part);
    CHECK(calls == before);
    llg_gmp_sv4_destroy(&merge);
    llg_gmp_sv4_destroy(&part);
    llg_gmp_sv4_destroy(&cat);
    llg_gmp_sv4_destroy(&stream);
    llg_gmp_sv4_destroy(&r);
    llg_gmp_sv4_destroy(&dst);
    llg_gmp_sv4_destroy(&source);
    puts("compact S4/S5 allocation checks passed");
    return 0;
}
