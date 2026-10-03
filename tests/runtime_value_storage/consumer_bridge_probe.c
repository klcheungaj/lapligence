#include "probe.h"
#include <string.h>

static size_t cases;
static unsigned pattern(uint32_t bit) { return (bit * 3u + bit / 7u) % 4u; }

static void check_ranges(void) {
    const uint32_t widths[] = {0, 1, 31, 32, 33, 63, 64, 65, 127, 128, 129, 257};
    for (size_t w = 0; w < sizeof(widths) / sizeof(widths[0]); ++w) {
        uint32_t width = widths[w];
        sv4_t source = sv4_zero(width, 1);
        for (uint32_t bit = 0; bit < width; ++bit)
            llg_sv4_set_state(&source, bit, pattern(bit));
        for (uint32_t offset = 0; offset <= 65; ++offset) {
            sv4_t target = sv4_fill(3, width + 130u, 0);
            size_t allocations = value_test_allocations();
            llg_sv4_range_copy(&target, offset, source);
            CHECK(llg_sv4_range_same(target, offset, source));
            for (uint32_t bit = 0; bit < width + 130u; ++bit)
                CHECK(llg_sv4_state(target, bit) ==
                      (bit >= offset && bit - offset < width ? pattern(bit - offset) : 3u));
            if (width) {
                llg_sv4_set_state(&target, offset + width - 1u, (pattern(width - 1u) + 1u) % 4u);
                CHECK(!llg_sv4_range_same(target, offset, source));
            }
            for (unsigned state = 0; state < 4; ++state) {
                llg_sv4_range_fill(&target, offset, width, state);
                for (uint32_t bit = offset; bit < offset + width; ++bit)
                    CHECK(llg_sv4_state(target, bit) == state);
            }
            llg_sv4_range_fill(&target, width + 128u, UINT32_MAX, 1);
            CHECK(llg_sv4_state(target, width + 129u) == 1);
            CHECK(value_test_allocations() == allocations);
            CHECK(llg_sv4_signed(target) == 0);
            sv4_destroy(&target);
            ++cases;
        }
        sv4_t mask = sv4_zero(width, 0);
        sv4_t removed = sv4_zero(width, 0);
        sv4_t target = sv4_fill(1, width, 0);
        for (uint32_t bit = 0; bit < width; ++bit) {
            llg_sv4_set_state(&mask, bit, bit % 2u);
            llg_sv4_set_state(&removed, bit, bit % 3u == 0);
        }
        size_t allocations = value_test_allocations();
        llg_sv4_masked_merge(&target, source, mask);
        for (uint32_t bit = 0; bit < width; ++bit)
            CHECK(llg_sv4_state(target, bit) == (bit % 2u ? pattern(bit) : 1u));
        llg_sv4_masked_merge(&target, target, mask);
        llg_sv4_range_copy(&source, 0, source);
        llg_sv4_mask_remove(&mask, removed);
        for (uint32_t bit = 0; bit < width; ++bit)
            CHECK(llg_sv4_state(mask, bit) == (bit % 2u && bit % 3u != 0));
        llg_sv4_two_state_inplace(&target);
        CHECK(!sv4_is_unknown(target));
        CHECK(value_test_allocations() == allocations);
        sv4_t wider = sv4_fill(3, width + 65u, 1);
        for (uint32_t bit = 0; bit < width; ++bit)
            llg_sv4_set_state(&mask, bit, bit % 4u);
        allocations = value_test_allocations();
        llg_sv4_masked_copy(&wider, source, mask);
        CHECK(llg_sv4_masked_same(wider, source, &mask));
        for (uint32_t bit = 0; bit < width + 65u; ++bit)
            CHECK(llg_sv4_state(wider, bit) ==
                  (bit < width && bit % 4u == 1u ? pattern(bit) : 3u));
        CHECK(llg_sv4_masked_same(source, source, NULL));
        llg_sv4_masked_copy(&wider, wider, mask);
        CHECK(value_test_allocations() == allocations);
        CHECK(llg_sv4_width(wider) == width + 65u && llg_sv4_signed(wider) == 1);
        sv4_destroy(&wider);
        sv4_destroy(&source);
        sv4_destroy(&mask);
        sv4_destroy(&removed);
        sv4_destroy(&target);
    }
}

static void check_boundaries(void) {
    struct { uint32_t aval, bval, sentinel; } words[10];
    sv4_t value = sv4_zero(257, 1);
    for (uint32_t bit = 0; bit < 257; ++bit)
        llg_sv4_set_state(&value, bit, bit % 4u);
    memset(words, 0xA5, sizeof(words));
    size_t allocations = value_test_allocations();
    llg_sv4_export_vpi32(value, words, 10, sizeof(words[0]));
    for (size_t i = 0; i < 8; ++i) {
        CHECK(words[i].aval == UINT32_C(0x66666666));
        CHECK(words[i].bval == UINT32_C(0xCCCCCCCC));
        CHECK(words[i].sentinel == UINT32_C(0xA5A5A5A5));
    }
    CHECK(words[8].aval == 0 && words[8].bval == 0);
    CHECK(words[9].aval == 0 && words[9].bval == 0);
    sv4_t imported = sv4_fill(3, 257, 0);
    allocations = value_test_allocations();
    llg_sv4_import_vpi32(&imported, words, 10, sizeof(words[0]));
    CHECK(sv4_same(value, imported));
    words[0].aval = UINT32_MAX;
    words[0].bval = 0;
    llg_sv4_import_vpi32(&imported, words, 1, sizeof(words[0]));
    for (uint32_t bit = 0; bit < 257; ++bit)
        CHECK(llg_sv4_state(imported, bit) == (bit < 32u ? 1u : bit % 4u));
    llg_sv4_vpi_word_t snapshot[5];
    llg_sv4_export_vpi_words(value, 0, snapshot, 5);
    CHECK(llg_sv4_same_vpi_words(value, snapshot, 5));
    snapshot[4].aval = 1;
    CHECK(!llg_sv4_same_vpi_words(value, snapshot, 5));
    char text[261];
    llg_sv4_export_text(value, 260, text);
    CHECK(strlen(text) == 260 && !memcmp(text, "000", 3));
    for (uint32_t bit = 0; bit < 257; ++bit)
        CHECK(text[259u - bit] == "01xz"[bit % 4u]);
    CHECK(value_test_allocations() == allocations);
    sv4_destroy(&value);
    sv4_destroy(&imported);
    ++cases;
}

static void check_digits(void) {
    const uint32_t widths[] = {0, 1, 7, 32, 63, 64, 65, 129};
    for (size_t i = 0; i < sizeof(widths) / sizeof(widths[0]); ++i) {
        uint32_t width = widths[i];
        sv4_t value = sv4_zero(width, 0);
        size_t allocations = value_test_allocations();
        const char* decimal = "18446744073709551616";
        for (const char* digit = decimal; *digit; ++digit)
            llg_sv4_mul_add_known(&value, 10, (uint32_t)(*digit - '0'));
        for (uint32_t bit = 0; bit < width; ++bit)
            CHECK(llg_sv4_state(value, bit) == (bit == 64));
        llg_sv4_negate_known(&value);
        for (uint32_t bit = 0; bit < width; ++bit)
            CHECK(llg_sv4_state(value, bit) == (bit >= 64));
        llg_sv4_range_fill(&value, 0, width, 0);
        for (unsigned digit = 0; digit < 80; ++digit)
            llg_sv4_append_digit(&value, 4, digit % 4u == 2u ? 2u : digit % 4u == 3u ? 3u : 0u,
                                 digit % 4u == 1u ? 15u : 0u);
        for (uint32_t bit = 0; bit < width; ++bit)
            CHECK(llg_sv4_state(value, bit) == (bit < 320u ? (79u - bit / 4u) % 4u : 0u));
        CHECK(value_test_allocations() == allocations);
        sv4_destroy(&value);
        ++cases;
    }
}

int main(void) {
    check_ranges();
    check_boundaries();
    check_digits();
    CHECK(value_test_live() == 0);
    printf("consumer bridge: %zu cases\n", cases);
    return 0;
}
