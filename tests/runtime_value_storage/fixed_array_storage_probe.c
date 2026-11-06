/* Descriptor ownership is independent of the logical 16M extent. */
#include "llg_rt.c"
#include "probe.h"
#include <string.h>

static void lifecycle(void) {
    for (unsigned round = 0; round < 100; ++round) {
        llg_rt_init();
        llg_fixed_array_t source = {0}, target = {0}, converted = {0};
        llg_fixed_array_init(&source, UINT64_C(16777216), sv4_x(129, 0), NULL);
        llg_fixed_array_init(&target, UINT64_C(16777216), sv4_x(129, 0), NULL);
        llg_fixed_array_init(&converted, UINT64_C(16777216), sv4_zero(129, 0), NULL);
        for (uint64_t index = 0; index < 1000; ++index)
            CHECK(sv4_is_unknown(*llg_fixed_array_peek(&source, index)));
        CHECK(source.count == 0);
        sv4_t* stable = llg_fixed_array_cell(&target, 0);
        sv4_t value = sv4_from_u64(7, 129, 0);
        llg_ba(llg_fixed_array_cell(&source, 0), value);
        sv4_destroy(&value);
        llg_fixed_array_copy(&target, &source, 0, 0);
        CHECK(stable == llg_fixed_array_cell(&target, 0));
        expect_number(sv4_clone(stable), 7);
        expect_number(llg_fixed_array_compare(&source, &target, 1, 0), 1);
        CHECK(sv4_is_unknown(*llg_fixed_array_peek(&target, 1)));
        llg_fixed_array_copy(&converted, &source, 1, 0);
        expect_number(sv4_clone(llg_fixed_array_peek(&converted, 1)), 0);
        llg_fixed_array_copy(&target, &source, 0, 1);
        value = sv4_from_u64(11, 129, 0);
        llg_ba(llg_fixed_array_cell(&source, 0), value);
        sv4_destroy(&value);
        llg_rt_run();
        expect_number(sv4_clone(stable), 7);
        llg_fixed_array_copy(&source, &source, 0, 0);
        expect_number(sv4_clone(llg_fixed_array_peek(&source, 0)), 11);
        llg_fixed_array_copy(&target, &source, 0, 1);
        llg_rt_cleanup(); /* Cancels and destroys the uncommitted snapshot. */
        llg_fixed_array_destroy(&source);
        llg_fixed_array_destroy(&target);
        llg_fixed_array_destroy(&converted);
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
    CHECK(value_test_peak_live() < 100);
}

static void stream_oracle(void) {
    llg_rt_init();
    llg_fixed_array_t source = {0}, target = {0};
    llg_fixed_array_init(&source, 5, sv4_zero(8, 0), NULL);
    llg_fixed_array_init(&target, 5, sv4_zero(8, 0), NULL);
    for (uint64_t i = 0; i < 5; ++i) {
        sv4_t value = sv4_from_u64(i + 1, 8, 0);
        llg_ba(llg_fixed_array_cell(&source, i), value);
        sv4_destroy(&value);
    }
    llg_fixed_array_stream_copy(&target, &source, 0, 0, 16);
    const uint64_t expected[5] = {4, 5, 2, 3, 1};
    for (uint64_t i = 0; i < 5; ++i)
        expect_number(sv4_clone(llg_fixed_array_peek(&target, i)), expected[i]);
    llg_rt_cleanup();
    llg_fixed_array_destroy(&source);
    llg_fixed_array_destroy(&target);
    CHECK(value_test_live() == 0);
}

/* Cells of both layouts reach their array's marker from their address, across
 * block boundaries, and inertial sites stay apart per cell and site. */
static void block_storage(void) {
    llg_rt_init();
    sv4_t marker = sv4_zero(1, 0);
    llg_fixed_array_t paged = {0}, sparse = {0};
    llg_fixed_array_init(&paged, 100, sv4_zero(8, 0), &marker);
    llg_fixed_array_init(&sparse, 5000, sv4_zero(8, 0), &marker);
    sv4_t* cells[104];
    const uint64_t positions[4] = {0, 31, 32, 99};
    for (size_t i = 0; i < 4; ++i) cells[i] = llg_fixed_array_cell(&paged, positions[i]);
    for (size_t i = 4; i < 104; ++i) cells[i] = llg_fixed_array_cell(&sparse, (i - 4) * 37);
    CHECK(paged.count == 4 && sparse.count == 100);
    for (size_t i = 0; i < 104; ++i) {
        uint64_t before = llg_sv4_word(marker, 0, LLG_SV4_BITS) & 1u;
        sv4_t value = sv4_from_u64(i + 1, 8, 0);
        llg_ba(cells[i], value);
        sv4_destroy(&value);
        CHECK((llg_sv4_word(marker, 0, LLG_SV4_BITS) & 1u) != before);
    }
    for (size_t i = 0; i < 4; ++i) {
        CHECK(llg_fixed_array_cell(&paged, positions[i]) == cells[i]);
        expect_number(sv4_clone(llg_fixed_array_peek(&paged, positions[i])), i + 1);
    }
    for (size_t i = 4; i < 104; ++i) {
        CHECK(llg_fixed_array_cell(&sparse, (i - 4) * 37) == cells[i]);
        expect_number(sv4_clone(llg_fixed_array_peek(&sparse, (i - 4) * 37)), i + 1);
    }
    expect_number(sv4_clone(llg_fixed_array_peek(&paged, 33)), 0);
    int first_site = 0, second_site = 0;
    llg_inertial_t** first = llg_fixed_array_inertial(cells[0], &first_site);
    CHECK(first == llg_fixed_array_inertial(cells[0], &first_site));
    CHECK(first != llg_fixed_array_inertial(cells[0], &second_site));
    CHECK(first != llg_fixed_array_inertial(cells[50], &first_site));
    CHECK(*first == NULL);
    llg_rt_cleanup();
    llg_fixed_array_destroy(&paged);
    llg_fixed_array_destroy(&sparse);
    sv4_destroy(&marker);
    /* Marker flips cache their two constant values for the process. */
    CHECK(value_test_live() <= 2);
}

int main(int argc, char** argv) {
    if (argc > 1 && strcmp(argv[1], "invalid-index") == 0) {
        llg_fixed_array_t value = {0};
        llg_fixed_array_init(&value, 1, sv4_zero(8, 0), NULL);
        (void)llg_fixed_array_cell(&value, UINT64_MAX);
        return 1;
    }
    lifecycle();
    stream_oracle();
    block_storage();
    puts("fixed array storage: OK");
    return 0;
}
