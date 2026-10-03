#ifndef COMPACT_ADAPTERS_TEST_H
#define COMPACT_ADAPTERS_TEST_H
#include "llg_value.h"
#include "backend.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static size_t checks;
#define CHECK(c) do { ++checks; if (!(c)) { fprintf(stderr, "%s:%d: %s\n", __FILE__, __LINE__, #c); abort(); } } while (0)
static inline void compare(sv4_t old, g4_t value) {
    CHECK(old.width == value.width && old.is_signed == value.is_signed);
    int unknown = 0;
    for (size_t i = 0; i < llg_sv4_words(old); ++i) {
        for (unsigned p = 0; p < 3; ++p)
            CHECK(llg_sv4_word(old, i, p) == llg_gmp_sv4_word(value, i, p));
        unknown |= (old.x[i] | old.z[i]) != 0;
    }
    CHECK(llg_gmp_sv4_is_unknown(value) == unknown);
    CHECK(llg_gmp_sv4_bytes(&value) == (value.width <= 64 ? 0 :
          8u * llg_gmp_sv4_words(value) * (unknown ? 2u : 1u)));
    sv4_destroy(&old);
    llg_gmp_sv4_destroy(&value);
}
static const uint32_t adapter_widths[] = {
    0, 1, 2, 7, 8, 31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257,
    1023, 1024, 4096, 8191, 8192, 8193, 16384, LLG_SUPPORTED_WIDTH_LIMIT - 1
};
static inline uint64_t next_word(uint64_t* seed) {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    return *seed;
}
#endif
