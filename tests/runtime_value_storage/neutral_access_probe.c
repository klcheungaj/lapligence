#ifdef LLG_VALUE_TEST_PROTOTYPE
#include "sv4.h"
#else
#include "llg_value.h"
#endif

#include <stdio.h>

static size_t checks;
#define CHECK(condition) do { \
    ++checks; \
    if (!(condition)) { \
        fprintf(stderr, "neutral value check failed at line %d: %s\n", \
                __LINE__, #condition); \
        return 1; \
    } \
} while (0)

static int check_words(void) {
    const uint32_t widths[] = {0, 1, 31, 32, 33, 63, 64, 65, 127, 128,
                              129, 257, 4097, 1048575};
    for (size_t n = 0; n < sizeof(widths) / sizeof(widths[0]); ++n) {
        uint32_t width = widths[n];
        sv4_t value = sv4_zero(width, 0);
        CHECK(llg_sv4_width(value) == width);
        CHECK(llg_sv4_words(value) == (width + 63u) / 64u);
        CHECK(llg_sv4_signed(value) == 0);
        llg_sv4_set_signed(&value, -7);
        CHECK(llg_sv4_signed(value) == 1);
        for (size_t i = 0; i < llg_sv4_words(value); ++i)
            llg_sv4_set_word(&value, i, UINT64_C(0x2222222222222222),
                             UINT64_C(0x4444444444444444),
                             UINT64_C(0x8888888888888888));
        for (uint64_t bit = 0; bit < width; ++bit)
            CHECK(llg_sv4_state(value, bit) == bit % 4u);
        CHECK(llg_sv4_has_x(value) == (width >= 3));
        CHECK(llg_sv4_has_z(value) == (width >= 4));
        CHECK(llg_sv4_state(value, width) == 2);
        CHECK(llg_sv4_state(value, UINT64_MAX) == 2);
        CHECK(llg_sv4_word(value, SIZE_MAX, LLG_SV4_BITS) == 0);
        llg_sv4_set_word(&value, SIZE_MAX, UINT64_MAX, 0, 0);
        llg_sv4_set_state(&value, UINT64_MAX, 1);
        size_t words = llg_sv4_words(value);
        if (words) {
            uint32_t remaining = width % 64u;
            uint64_t mask = remaining ? UINT64_MAX >> (64u - remaining) : UINT64_MAX;
            CHECK(llg_sv4_word(value, words - 1u, LLG_SV4_BITS) ==
                  (UINT64_C(0x2222222222222222) & mask));
            CHECK(llg_sv4_word(value, words - 1u, LLG_SV4_X) ==
                  (UINT64_C(0x4444444444444444) & mask));
            CHECK(llg_sv4_word(value, words - 1u, LLG_SV4_Z) ==
                  (UINT64_C(0x8888888888888888) & mask));
            for (unsigned state = 0; state < 4; ++state) {
                llg_sv4_set_state(&value, width - 1u, state);
                CHECK(llg_sv4_state(value, width - 1u) == state);
            }
        }
        llg_sv4_word_t padded[2] = {{1, 2, 4}, {1, 2, 4}};
        llg_sv4_vpi_word_t vpi_padded[2] = {{1, 2}, {1, 2}};
        llg_sv4_export_words(value, SIZE_MAX, padded, 2);
        llg_sv4_export_vpi_words(value, SIZE_MAX, vpi_padded, 2);
        CHECK(padded[0].bits == 0 && padded[1].bits == 0);
        CHECK(padded[0].x == 0 && padded[1].z == 0);
        CHECK(vpi_padded[0].aval == 0 && vpi_padded[1].bval == 0);
        llg_sv4_import_words(&value, SIZE_MAX, padded, 2);
        llg_sv4_import_vpi_words(&value, SIZE_MAX, vpi_padded, 2);
        llg_sv4_export_words(value, 0, NULL, 0);
        llg_sv4_import_words(&value, 0, NULL, 0);
        llg_sv4_export_vpi_words(value, 0, NULL, 0);
        llg_sv4_import_vpi_words(&value, 0, NULL, 0);
        sv4_destroy(&value);
        CHECK(llg_sv4_width(value) == 0);
    }
    return 0;
}

static int check_encodings(void) {
    const unsigned dpi_codes[] = {0, 1, 3, 2};
    sv4_t value = sv4_zero(129, 0);
    const llg_sv4_word_t input[] = {{2, 4, 8}, {2, 4, 8}, {UINT64_MAX, 0, 0}};
    llg_sv4_import_words(&value, 0, input, 3);
    for (size_t word = 0; word < 2; ++word) {
        for (unsigned state = 0; state < 4; ++state) {
            CHECK(llg_sv4_state(value, 64u * word + state) == state);
            CHECK(llg_sv4_state_to_dpi(state) == dpi_codes[state]);
            CHECK(llg_sv4_state_from_dpi(dpi_codes[state]) == state);
        }
        llg_sv4_vpi_word_t encoded = llg_sv4_vpi_word(value, word);
        CHECK(encoded.aval == 6 && encoded.bval == 12);
    }
    CHECK(llg_sv4_word(value, 2, LLG_SV4_BITS) == 1);
    const uint64_t rust_value = 10, rust_unknown = 12;
    llg_sv4_set_word(&value, 0, rust_value & ~rust_unknown,
                     rust_unknown & ~rust_value, rust_unknown & rust_value);
    for (unsigned state = 0; state < 4; ++state)
        CHECK(llg_sv4_state(value, state) == state);
    llg_sv4_word_t copied[4];
    llg_sv4_vpi_word_t encoded[4];
    llg_sv4_export_words(value, 0, copied, 4);
    llg_sv4_export_vpi_words(value, 0, encoded, 4);
    CHECK(copied[0].bits == 2 && copied[0].x == 4 && copied[0].z == 8);
    CHECK(encoded[0].aval == 6 && encoded[0].bval == 12);
    CHECK(copied[2].bits == 1 && copied[3].bits == 0);
    CHECK(encoded[2].aval == 1 && encoded[3].bval == 0);
    sv4_t roundtrip = sv4_zero(129, 1);
    llg_sv4_import_vpi_words(&roundtrip, 0, encoded, 4);
    CHECK(sv4_same(value, roundtrip));
    CHECK(llg_sv4_signed(roundtrip) == 1);
    const llg_sv4_vpi_word_t partial[] = {{UINT64_MAX, UINT64_MAX}, {0, 0}};
    llg_sv4_import_vpi_words(&roundtrip, 2, partial, 2);
    CHECK(llg_sv4_state(roundtrip, 128) == 2);
    CHECK(llg_sv4_state(roundtrip, 64) == 0);
    llg_sv4_set_word(&roundtrip, 0, UINT64_MAX, 4, 12);
    CHECK(llg_sv4_word(roundtrip, 0, LLG_SV4_X) == 4);
    CHECK(llg_sv4_word(roundtrip, 0, LLG_SV4_Z) == 8);
    CHECK(llg_sv4_word(roundtrip, 0, LLG_SV4_BITS) == (UINT64_MAX & ~UINT64_C(12)));
    sv4_destroy(&roundtrip);
    sv4_destroy(&value);
    return 0;
}

static int check_extension(void) {
    const uint32_t widths[] = {1, 7, 64, 65, 129};
    for (unsigned source_sign = 0; source_sign < 2; ++source_sign) {
        for (unsigned result_sign = 0; result_sign < 2; ++result_sign) {
            for (unsigned state = 0; state < 4; ++state) {
                sv4_t source = sv4_fill((uint8_t)state, 1, (int8_t)source_sign);
                for (size_t i = 0; i < sizeof(widths) / sizeof(widths[0]); ++i) {
                    sv4_t cast = sv4_cast(source, widths[i], (int8_t)result_sign);
                    sv4_t resized = sv4_resize(source, widths[i], (int8_t)result_sign);
                    for (uint32_t bit = 0; bit < widths[i]; ++bit) {
                        CHECK(llg_sv4_state(cast, bit) ==
                              (bit == 0 || source_sign ? state : 0));
                        CHECK(llg_sv4_state(resized, bit) ==
                              (bit == 0 || result_sign ? state : 0));
                    }
                    CHECK(llg_sv4_signed(cast) == (int8_t)result_sign);
                    CHECK(llg_sv4_signed(resized) == (int8_t)result_sign);
                    sv4_t coerced = sv4_to_two_state(cast);
                    for (uint32_t bit = 0; bit < widths[i]; ++bit)
                        CHECK(llg_sv4_state(coerced, bit) ==
                              (state == 1 && (bit == 0 || source_sign) ? 1u : 0u));
                    CHECK(!sv4_is_unknown(coerced));
                    CHECK(llg_sv4_width(coerced) == widths[i]);
                    CHECK(llg_sv4_signed(coerced) == (int8_t)result_sign);
                    llg_sv4_set_state(&cast, 0, (state + 1u) % 4u);
                    CHECK(llg_sv4_state(source, 0) == state);
                    CHECK(llg_sv4_state(resized, 0) == state);
                    sv4_destroy(&coerced);
                    sv4_destroy(&resized);
                    sv4_destroy(&cast);
                }
                sv4_destroy(&source);
            }
        }
    }
    sv4_t source = sv4_from_u64(255, 8, 1);
    sv4_t narrowed = sv4_cast(source, 7, 0);
    CHECK(sv4_to_u64(narrowed) == 127 && llg_sv4_signed(narrowed) == 0);
    sv4_destroy(&narrowed);
    sv4_destroy(&source);
    return 0;
}

static int check_owners(void) {
    sv4_t literal = SV4_INIT(2, 4, 8, 4, 0);
    CHECK(llg_sv4_state(literal, 1) == 1 && llg_sv4_state(literal, 2) == 2);
    sv4_destroy(&literal);
    sv4_replace(&literal, SV4_C(7, 4));
    CHECK(sv4_to_u64(literal) == 7 && llg_sv4_signed(literal) == 0);
    sv4_replace(&literal, SV4_S(7, 4));
    CHECK(sv4_to_u64(literal) == 7 && llg_sv4_signed(literal) == 1);
    sv4_replace(&literal, SV4_X(4));
    CHECK(llg_sv4_has_x(literal) && !llg_sv4_has_z(literal));
    sv4_replace(&literal, SV4_Z(4));
    CHECK(llg_sv4_has_z(literal) && !llg_sv4_has_x(literal));
    /* Run-time widths: with a constant 64, MSVC diagnoses the shift in the
     * macro's untaken branch (C4293). */
    const uint32_t mask_widths[] = {0, 64};
    CHECK(LLG_MASK(mask_widths[0]) == 0 && LLG_MASK(mask_widths[1]) == UINT64_MAX);
    sv4_destroy(&literal);
    const uint32_t widths[] = {1, 64, 65, 129};
    for (size_t i = 0; i < sizeof(widths) / sizeof(widths[0]); ++i) {
        sv4_t source = sv4_from_u64(1, widths[i], 0);
        sv4_t clone = sv4_clone(&source);
        sv4_t destination = SV4_EMPTY;
        sv4_copy(&destination, &source);
        sv4_copy(&destination, &destination);
        sv4_move(&destination, &destination);
        llg_sv4_set_state(&clone, 0, 3);
        CHECK(llg_sv4_state(source, 0) == 1);
        CHECK(llg_sv4_state(destination, 0) == 1);
        sv4_assign(&destination, destination);
        sv4_move(&destination, &clone);
        CHECK(llg_sv4_width(clone) == 0);
        CHECK(llg_sv4_state(destination, 0) == 3);
        sv4_replace(&destination, sv4_to_two_state(destination));
        CHECK(llg_sv4_state(destination, 0) == 0);
        CHECK(llg_sv4_state(source, 0) == 1);
        sv4_t sum = sv4_add(source, source);
        CHECK(sv4_to_u64(sum) == (widths[i] == 1 ? 0u : 2u));
        llg_sv4_set_state(&sum, 0, 2);
        CHECK(llg_sv4_state(source, 0) == 1);
        sv4_destroy(&sum);
        sv4_destroy(&destination);
        sv4_destroy(&clone);
        sv4_destroy(&source);
        sv4_destroy(&source);
    }
    return 0;
}

int main(void) {
    if (check_words() || check_encodings() || check_extension() || check_owners()) return 1;
    printf("PASS: %zu neutral access, encoding, extension and ownership checks\n", checks);
    return 0;
}
