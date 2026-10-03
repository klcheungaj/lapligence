#include "llg_value.h"
#include "llg_vpi.h"
#undef NDEBUG
#include <assert.h>
#include <stdio.h>

int main(void) {
    assert(sizeof(sv4_t) == (LLG_SV4_USE_GMP ? 24 : 32));
    assert(_Alignof(sv4_t) == 8);
    assert(sizeof(llg_vpi_arg_t) == (LLG_SV4_USE_GMP ? 48 : 56));
    assert(_Alignof(llg_vpi_arg_t) == 8);
    sv4_t value = sv4_fill(2, 129, 1);
    sv4_t source = sv4_from_u64(42, 129, 0);
    sv4_t mask = sv4_fill(1, 129, 0);
    llg_sv4_masked_copy(&value, source, mask);
    assert(!sv4_is_unknown(value));
    assert(llg_sv4_signed(value) && sv4_to_u64(value) == 42);
    llg_sv4_range_fill(&value, 63, 65, 3);
    assert(llg_sv4_state(value, 62) == 0 && llg_sv4_state(value, 63) == 3);
    assert(llg_sv4_state(value, 127) == 3 && llg_sv4_state(value, 128) == 0);
    llg_sv4_range_copy(&value, 0, source);
    assert(llg_sv4_range_same(value, 0, source));
    sv4_replace(&source, sv4_fill(2, 129, 0));
    llg_sv4_masked_copy(&source, value, mask);
    assert(!sv4_is_unknown(source) && sv4_to_u64(source) == 42);
    llg_sv4_masked_copy(&source, source, mask);
    llg_sv4_masked_merge(&source, source, source);
    llg_sv4_mul_add_known(&source, 3, 7);
    assert(sv4_to_u64(source) == 133);
    llg_sv4_negate_known(&source);
    assert(sv4_to_u64(source) == UINT64_C(0) - 133);
    llg_sv4_two_state_inplace(&source);
    llg_sv4_append_digit(&value, 4, 0, 7);
    assert(sv4_to_u64(value) == 679);
    llg_sv4_vpi_word_t snapshot[3];
    llg_sv4_export_vpi_words(value, 0, snapshot, 3);
    assert(llg_sv4_same_vpi_words(value, snapshot, 3));
    uint32_t foreign[10];
    llg_sv4_export_vpi32(value, foreign, 5, 8);
    llg_sv4_import_vpi32(&source, foreign, 5, 8);
    assert(sv4_same(source, value));
    char text[130];
    llg_sv4_export_text(value, 129, text);
    assert(text[128] == '1' && text[129] == 0);
    llg_sv4_set_word(&source, 1, UINT64_MAX, 0, 0);
    llg_sv4_set_state(&source, 0, 2);
    llg_sv4_two_state_inplace(&source);
    llg_sv4_mask_remove(&source, source);
    assert(!sv4_is_unknown(source) && sv4_to_u64(source) == 0);
    assert(llg_sv4_word(source, 1, LLG_SV4_BITS) == 0);
    sv4_destroy(&value);
    sv4_destroy(&source);
    sv4_destroy(&mask);
    puts("selected facade passed");
}
