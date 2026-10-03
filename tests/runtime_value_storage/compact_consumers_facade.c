#define LLG_SV4_GMP_PUBLIC_NAMES
#include "backend.h"
#include <stdlib.h>
#define CHECK(c)                                                                                   \
    do {                                                                                           \
        if (!(c))                                                                                  \
            abort();                                                                               \
    } while (0)
int main(void) {
    sv4_t v = SV4_C(19, 65), source = SV4_C(3, 4), mask = SV4_C(15, 65);
    llg_sv4_masked_merge(&v, source, mask);
    CHECK(llg_sv4_masked_same(v, source, &mask));
    llg_sv4_masked_copy(&v, source, mask);
    llg_sv4_range_copy(&v, 31, source);
    CHECK(llg_sv4_range_same(v, 31, source));
    llg_sv4_range_fill(&v, 64, 1, 2);
    CHECK(llg_sv4_plane_slice(v, 64, 1, LLG_SV4_X) == 1);
    llg_sv4_two_state_inplace(&v);
    llg_sv4_mask_remove(&v, mask);
    llg_sv4_mul_add_known(&v, 10, 7);
    llg_sv4_negate_known(&v);
    llg_sv4_append_digit(&v, 4, 3, 0);
    llg_sv4_mask_top(&v);
    llg_sv4_vpi_word_t words[3];
    llg_sv4_export_vpi_words(v, 0, words, 3);
    CHECK(llg_sv4_same_vpi_words(v, words, 3));
    unsigned char records[40];
    llg_sv4_export_vpi32(v, records, 5, 8);
    sv4_t copied = sv4_zero(65, 0);
    llg_sv4_import_vpi32(&copied, records, 5, 8);
    CHECK(sv4_same(v, copied));
    char text[68];
    llg_sv4_export_text(v, 67, text);
    CHECK(text[0] == '0' && text[1] == '0' && text[66] == 'z' && !text[67]);
    sv4_destroy(&v);
    sv4_destroy(&source);
    sv4_destroy(&mask);
    sv4_destroy(&copied);
    return 0;
}
