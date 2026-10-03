#include "backend.h"
#include <stdio.h>
#include <stdlib.h>

static size_t allocations, reallocations;
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
            fprintf(stderr, "destination allocation %d: %s\n", __LINE__, #c);                      \
            abort();                                                                               \
        }                                                                                          \
    } while (0)
static void (*ops[])(g4_t*, g4_t, g4_t) = {llg_gmp_sv4_add_into, llg_gmp_sv4_sub_into,
                                           llg_gmp_sv4_mul_into};
int main(void) {
    uint32_t widths[] = {0, 1, 8, 63, 64, 65, 256, 4096, 8128, 8129, 8192, 8193};
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k)
        for (unsigned op = 0; op < 3; ++op)
            for (unsigned alias = 0; alias < 4; ++alias) {
                uint32_t w = widths[k];
                g4_t a = llg_gmp_sv4_fill(1, w, 1), b = llg_gmp_sv4_from_u64(3, w, 0),
                     d = llg_gmp_sv4_zero(w, 1);
                g4_t* target = alias == 1 || alias == 3 ? &a : alias == 2 ? &b : &d;
                uint64_t* payload = w > 64 ? target->data.wide.a : NULL;
                size_t before = allocations, rb = reallocations;
                ops[op](target, a, alias == 3 ? a : b);
                if (w <= 64 || op < 2) {
                    CHECK(allocations == before && reallocations == rb);
                    if (w > 64)
                        CHECK(target->data.wide.a == payload);
                } else if (alias) {
                    CHECK(allocations == before + 1);
                    CHECK(target->data.wide.a != payload);
                } else {
                    CHECK(target->data.wide.a == payload);
                    CHECK(reallocations == rb);
                    CHECK(allocations <= before + 1);
                    if (w <= 4096)
                        CHECK(allocations == before);
                }
                llg_gmp_sv4_destroy(&a);
                llg_gmp_sv4_destroy(&b);
                llg_gmp_sv4_destroy(&d);
            }
    for (unsigned op = 0; op < 3; ++op) {
        g4_t a = llg_gmp_sv4_x(256, 1), b = llg_gmp_sv4_fill(3, 256, 1),
             d = llg_gmp_sv4_zero(256, 0);
        size_t before = allocations, rb = reallocations;
        ops[op](&d, a, b);
        CHECK(allocations == before && reallocations == rb + 1);
        rb = reallocations;
        ops[op](&d, d, d);
        CHECK(allocations == before && reallocations == rb);
        CHECK(llg_gmp_sv4_has_x(d) && !llg_gmp_sv4_has_z(d));
        llg_gmp_sv4_destroy(&a);
        llg_gmp_sv4_destroy(&b);
        a = llg_gmp_sv4_from_u64(3, 256, 1);
        b = llg_gmp_sv4_from_u64(5, 256, 0);
        before = allocations;
        rb = reallocations;
        ops[op](&d, a, b);
        CHECK(allocations == before && reallocations == rb + 1);
        CHECK(!d.data.wide.b && !d.is_signed);
        llg_gmp_sv4_destroy(&a);
        llg_gmp_sv4_destroy(&b);
        llg_gmp_sv4_destroy(&d);
    }
    puts("destination storage reuse, alias scratch and B transitions passed");
    return 0;
}
