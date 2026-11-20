#include "llg_rt.c"
#include "probe.h"

/* Dense sampled cell images (sampling.c) copy at a Preponed sample only the
 * cells written since the previous one, and the sampled save table of a
 * descriptor array (fixed_arrays.c) returns to its initial capacity once
 * bursts of writes stop. */

#define CELLS 64u
#define DESCRIPTOR_CELLS 5000u

static sv4_t first[CELLS], second[CELLS], outside;
static double reals[4];

/* The Preponed sample of the next time slot. */
static void next_slot(void) {
    g.now++;
    sample_preponed_values();
}

static void write_cell(sv4_t* cell, uint64_t value) {
    sv4_t owned = sv4_from_u64(value, 8, 0);
    llg_ba(cell, owned);
    sv4_destroy(&owned);
}

static void dense_images(void) {
    llg_rt_init();
    for (unsigned i = 0; i < CELLS; i++) {
        first[i] = sv4_from_u64(i, 8, 0);
        second[i] = sv4_from_u64(100u + i, 8, 0);
    }
    outside = sv4_zero(8, 0);
    for (unsigned i = 0; i < 4; i++) reals[i] = 0.5 + i;
    /* Registered out of address order: the lookup index sorts them. */
    const sv4_t* b = llg_sampled_cells(second, CELLS);
    const sv4_t* a = llg_sampled_cells(first, CELLS);
    const double* r = llg_sampled_real_cells(reals, 4);
    CHECK(a && b && r);
    CHECK(llg_sampled_cells(first, CELLS) == a);
    CHECK(g.sampled_cell_index_count == 3 && fixed_marked_arrays == 3);
    for (size_t i = 1; i < g.sampled_cell_index_count; i++)
        CHECK(g.sampled_cell_index[i - 1]->base < g.sampled_cell_index[i]->base);
    llg_sampled_cells_t* fa = sampled_cells_registered((uintptr_t)first);
    llg_sampled_cells_t* fb = sampled_cells_registered((uintptr_t)second);
    llg_sampled_cells_t* fr = sampled_cells_registered((uintptr_t)reals);
    CHECK(fa && fb && fr && fa->image == a && fb->image == b && fr->real_image == r);

    /* A store before the first sample that publishes nothing, as an
     * initializer does, still reaches the image: that sample copies all. */
    sv4_replace(&first[5], sv4_from_u64(55, 8, 0));
    next_slot();
    expect_number(sv4_clone(&a[5]), 55);
    CHECK(!fa->whole && fa->changed_count == 0);

    /* Each written cell is listed once; a signal outside the images is not. */
    write_cell(&first[3], 33);
    write_cell(&first[3], 34);
    write_cell(&second[CELLS - 1], 7);
    write_cell(&outside, 1);
    llg_ba_d(&reals[2], 9.0);
    CHECK(fa->changed_count == 1 && fa->changed[0] == 3 && fa->listed[3]);
    CHECK(fb->changed_count == 1 && fb->changed[0] == CELLS - 1);
    CHECK(fr->changed_count == 1 && fr->changed[0] == 2);
    /* Until the next sample the image keeps the slot's Preponed values. */
    expect_number(sv4_clone(&a[3]), 3);
    expect_number(sv4_clone(&b[CELLS - 1]), 100u + CELLS - 1);
    CHECK(r[2] == 2.5);
    next_slot();
    expect_number(sv4_clone(&a[3]), 34);
    expect_number(sv4_clone(&b[CELLS - 1]), 7);
    CHECK(r[2] == 9.0);
    CHECK(fa->changed_count == 0 && !fa->listed[3] && fb->changed_count == 0 &&
          fr->changed_count == 0);

    /* A slot without writes copies nothing and keeps every value. */
    next_slot();
    expect_number(sv4_clone(&a[3]), 34);
    expect_number(sv4_clone(&a[5]), 55);

    /* Every cell written twice in one slot: the list stays within the array. */
    for (unsigned round = 0; round < 2; round++)
        for (unsigned i = 0; i < CELLS; i++) write_cell(&first[i], 128u + round + i);
    CHECK(fa->changed_count == CELLS);
    next_slot();
    for (unsigned i = 0; i < CELLS; i++) expect_number(sv4_clone(&a[i]), 129u + i);

    /* Stores that publish nothing (a quiet VPI put) are listed too. */
    llg_store_unpublished(&first[7], sv4_from_u64(77, 8, 0));
    llg_store_unpublished_d(&reals[0], 4.0);
    CHECK(fa->changed_count == 1 && fr->changed_count == 1);
    next_slot();
    expect_number(sv4_clone(&a[7]), 77);
    CHECK(r[0] == 4.0);

    llg_rt_cleanup();
    CHECK(g.sampled_cells == NULL && g.sampled_cell_index == NULL &&
          g.sampled_cell_index_count == 0);
    CHECK(fixed_marked_arrays == 0 && fixed_dense_low == UINTPTR_MAX && fixed_dense_high == 0);
    CHECK(fixed_written_low == UINTPTR_MAX && fixed_written_high == 0);
    for (unsigned i = 0; i < CELLS; i++) {
        sv4_destroy(&first[i]);
        sv4_destroy(&second[i]);
    }
    sv4_destroy(&outside);
}

static llg_fixed_array_t descriptor;
static uint8_t shadow[DESCRIPTOR_CELLS];

/* Changes cell `index` in the current slot: the sampled value is the one
 * before the change. */
static void change(uint64_t index) {
    uint8_t old = shadow[index];
    shadow[index] = (uint8_t)(old % 250u + 1u);
    write_cell(llg_fixed_array_cell(&descriptor, index), shadow[index]);
    expect_number(sv4_clone(llg_fixed_array_sampled_peek(&descriptor, index)), old);
    expect_number(sv4_clone(llg_fixed_array_peek(&descriptor, index)), shadow[index]);
}

static void burst(void) {
    next_slot();
    for (uint64_t i = 0; i < DESCRIPTOR_CELLS; i++) change(i);
}

/* One slot that changes `count` cells, then the capacity the table has. */
static size_t slot_changing(uint64_t count) {
    next_slot();
    for (uint64_t i = 0; i < count; i++) change((g.now * 7u + i) % DESCRIPTOR_CELLS);
    return descriptor.sampled->capacity;
}

static void save_table_shrinks(void) {
    llg_rt_init();
    llg_fixed_array_init(&descriptor, DESCRIPTOR_CELLS, sv4_zero(8, 0), NULL);
    llg_fixed_array_sampled(&descriptor);
    burst();
    const size_t peak = descriptor.sampled->capacity;
    CHECK(peak == 16384u);
    /* The slot after the burst resets nothing; then each run of
     * LLG_FIXED_SAMPLED_SHRINK_SLOTS sparse slots halves the table, down to
     * its initial capacity. */
    for (uint64_t slot = 1; slot <= 64u * 12u; slot++) {
        size_t halvings = (size_t)((slot - 1u) / LLG_FIXED_SAMPLED_SHRINK_SLOTS);
        size_t expected = halvings >= 10u ? LLG_FIXED_INDEX_INITIAL_CAPACITY : peak >> halvings;
        CHECK(slot_changing(1) == expected);
    }
    /* A second burst grows it again, and a slot filling an eighth of the
     * table restarts the count. */
    burst();
    CHECK(descriptor.sampled->capacity == peak);
    for (unsigned slot = 1; slot <= 62u; slot++) CHECK(slot_changing(1) == peak);
    CHECK(slot_changing(peak / LLG_FIXED_SAMPLED_SHRINK_FILL) == peak);
    for (unsigned slot = 0; slot < 64u; slot++) CHECK(slot_changing(1) == peak);
    CHECK(slot_changing(1) == peak / 2u);
    llg_rt_cleanup();
    llg_fixed_array_destroy(&descriptor);
    CHECK(value_test_live() <= 2);
}

int main(void) {
    dense_images();
    save_table_shrinks();
    puts("sampled cells: OK");
    return 0;
}
