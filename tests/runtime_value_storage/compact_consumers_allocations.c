#define LLG_SV4_GMP_PUBLIC_NAMES
#include "backend.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
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
void* __wrap_malloc(size_t n) {
    ++calls;
    return __real_malloc(n);
}
void* __wrap_calloc(size_t n, size_t s) {
    ++calls;
    return __real_calloc(n, s);
}
void* __wrap_realloc(void* p, size_t n) {
    ++calls;
    return __real_realloc(p, n);
}
int main(void) {
    uint32_t widths[] = {0, 1, 4, 32, 64, 65, 256, 4096};
    llg_sv4_vpi_word_t words[65];
    unsigned char raw[1056];
    char text[4164];
    for (size_t k = 0; k < sizeof(widths) / sizeof(widths[0]); ++k) {
        uint32_t w = widths[k];
        sv4_t v = sv4_fill(1, w, 1), known = sv4_zero(w, 0), mask = sv4_fill(1, w, 0);
        size_t before = calls;
        llg_sv4_export_vpi_words(v, 0, words, 65);
        CHECK(llg_sv4_same_vpi_words(v, words, 65));
        llg_sv4_export_vpi32(v, raw, 132, 8);
        llg_sv4_import_vpi32(&v, raw, 132, 8);
        llg_sv4_export_text(v, w + 67, text);
        CHECK(llg_sv4_masked_same(v, v, NULL));
        CHECK(llg_sv4_range_same(v, 0, v));
        llg_sv4_range_copy(&v, 0, v);
        llg_sv4_masked_merge(&v, known, v);
        llg_sv4_masked_copy(&v, known, mask);
        llg_sv4_range_copy(&v, 0, known);
        llg_sv4_mask_remove(&v, v);
        llg_sv4_range_fill(&v, 0, UINT32_MAX, 1);
        llg_sv4_mul_add_known(&v, UINT32_MAX, UINT32_MAX);
        llg_sv4_negate_known(&v);
        llg_sv4_append_digit(&v, 4, 0, 15);
        llg_sv4_two_state_inplace(&v);
        llg_sv4_mask_top(&v);
        CHECK(calls == before);
        if (w <= 64) {
            for (unsigned s = 0; s < 4; ++s) {
                llg_sv4_range_fill(&v, 0, UINT32_MAX, s);
                llg_sv4_append_digit(&v, 1, s, 1);
                llg_sv4_export_vpi32(v, raw, 132, 8);
                llg_sv4_import_vpi32(&v, raw, 132, 8);
                llg_sv4_two_state_inplace(&v);
            }
            CHECK(calls == before);
        } else {
            sv4_t unknown = sv4_zero(w, 0);
            llg_sv4_range_fill(&unknown, w - 1, 1, 2);
            sv4_t low_mask = SV4_C(1, w);
            before = calls;
            llg_sv4_range_fill(&v, w, UINT32_MAX, 2);
            llg_sv4_range_fill(&v, 0, 0, 3);
            llg_sv4_masked_merge(&v, unknown, low_mask);
            llg_sv4_masked_copy(&v, unknown, low_mask);
            CHECK(calls == before && (size_t)llg_gmp_sv4_bytes(&v) == 8 * llg_sv4_words(v));
            llg_sv4_masked_copy(&v, unknown, mask);
            CHECK(calls == before + 1 && (size_t)llg_gmp_sv4_bytes(&v) == 16 * llg_sv4_words(v));
            llg_sv4_masked_merge(&v, unknown, mask);
            CHECK(calls == before + 1);
            llg_sv4_range_fill(&v, w - 1, 1, 0);
            CHECK(calls == before + 2 && (size_t)llg_gmp_sv4_bytes(&v) == 8 * llg_sv4_words(v));
            before = calls;
            llg_sv4_range_copy(&v, 0, unknown);
            CHECK(calls == before + 1);
            llg_sv4_append_digit(&v, 1, 0, 0);
            CHECK(calls == before + 2 && (size_t)llg_gmp_sv4_bytes(&v) == 8 * llg_sv4_words(v));
            before = calls;
            llg_sv4_append_digit(&v, 4, 3, 0);
            CHECK(calls == before + 1);
            llg_sv4_two_state_inplace(&v);
            CHECK(calls == before + 2);
            /* Odd record import preserves high halves; B in padding is ignored. */
            memset(raw, 0, sizeof(raw));
            size_t record = (w - 1u) / 32u;
            uint32_t b = UINT32_C(1) << ((w - 1u) % 32u);
            memcpy(raw + record * 8 + 4, &b, 4);
            before = calls;
            llg_sv4_import_vpi32(&v, raw, record + 1, 8);
            CHECK(calls == before + 1);
            memset(raw, 0, sizeof(raw));
            llg_sv4_import_vpi32(&v, raw, record + 1, 8);
            CHECK(calls == before + 2);
            if (w % 32u) {
                b = UINT32_MAX << (w % 32u);
                memcpy(raw + record * 8 + 4, &b, 4);
                before = calls;
                llg_sv4_import_vpi32(&v, raw, record + 1, 8);
                CHECK(calls == before);
            }
            /* Destination-as-mask survives the one required promotion. */
            llg_sv4_range_fill(&v, 0, UINT32_MAX, 1);
            before = calls;
            llg_sv4_masked_merge(&v, unknown, v);
            CHECK(calls == before + 1 && llg_sv4_state(v, w - 1) == 2);
            sv4_destroy(&unknown);
            sv4_destroy(&low_mask);
        }
        sv4_destroy(&v);
        sv4_destroy(&known);
        sv4_destroy(&mask);
    }
    puts("compact V06 bridge allocation checks passed");
    return 0;
}
