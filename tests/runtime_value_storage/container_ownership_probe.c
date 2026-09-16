#include "llg_container.c"
#include "probe.h"

static void check_dynamic_array(void) {
    llg_dyn_array_t array, copy;
    llg_dyn_init(&array, 129, 0, 0);
    llg_dyn_init(&copy, 129, 0, 0);
    sv4_t count = sv4_from_u64(3, 32, 0);
    sv4_t index = sv4_zero(32, 0);
    sv4_t value = sv4_from_u64(17, 129, 0);
    llg_dyn_new(&array, count, NULL);
    CHECK(llg_dyn_set(&array, index, value));
    value.bits[0] = 99;
    expect_number(llg_dyn_get(&array, index), 17);
    llg_dyn_copy(&copy, &array);
    llg_dyn_copy(&array, &array);
    sv4_t read = llg_dyn_get(&copy, index);
    read.bits[0] = 32;
    expect_number(llg_dyn_get(&copy, index), 17);
    sv4_destroy(&read);
    size_t retained = value_test_live();
    for (unsigned i = 0; i < 1000; ++i) {
        llg_dyn_set(&array, index, value);
        sv4_t reduced = llg_dyn_reduce(&array, LLG_CONTAINER_REDUCE_SUM);
        sv4_destroy(&reduced);
        llg_dyn_copy(&copy, &array);
        CHECK(value_test_live() == retained);
    }
    llg_dyn_destroy(&array);
    llg_dyn_destroy(&copy);
    sv4_destroy(&count);
    sv4_destroy(&index);
    sv4_destroy(&value);
    CHECK(value_test_live() == 0);
}

static void check_pinned_queue(void) {
    llg_queue_t queue;
    llg_queue_init(&queue, 65, 0, 0, UINT64_MAX);
    sv4_t value = sv4_from_u64(11, 65, 0);
    llg_queue_push_back(&queue, value);
    void* pinned = llg_queue_ref_acquire(&queue, 0);
    void* again = llg_queue_ref_acquire(&queue, 0);
    CHECK(pinned == again);
    value.bits[0] = 22;
    llg_queue_push_front(&queue, value);
    expect_number(llg_queue_cell_read(pinned), 11);
    expect_number(llg_queue_pop_back(&queue), 11);
    value.bits[0] = 33;
    CHECK(llg_queue_cell_write(pinned, value));
    expect_number(llg_queue_cell_read(pinned), 33);
    expect_number(llg_queue_front(&queue), 22);
    llg_queue_destroy(&queue);
    expect_number(llg_queue_cell_read(pinned), 33);
    llg_queue_ref_release(again);
    llg_queue_ref_release(pinned);
    llg_queue_init(&queue, 65, 0, 0, UINT64_MAX);
    size_t retained = value_test_live();
    for (unsigned i = 0; i < 1000; ++i) {
        llg_queue_push_front(&queue, value);
        llg_queue_push_back(&queue, value);
        llg_queue_copy(&queue, &queue);
        expect_number(llg_queue_pop_front(&queue), 33);
        expect_number(llg_queue_pop_back(&queue), 33);
        CHECK(value_test_live() == retained);
    }
    llg_queue_destroy(&queue);
    sv4_destroy(&value);
    CHECK(value_test_live() == 0);
}

static void check_associative_owners(void) {
    llg_assoc_t array, copy;
    llg_assoc_init_integral(&array, 129, 0, 0, 0, 0, 0);
    llg_assoc_init_integral(&copy, 129, 0, 0, 0, 0, 0);
    sv4_t short_key = sv4_from_i64(-1, 8);
    sv4_t wide_key = sv4_from_i64(-1, 4096);
    sv4_t positive = sv4_from_u64(255, 8, 0);
    sv4_t absent = sv4_from_u64(9, 8, 0);
    sv4_t value = sv4_from_u64(42, 129, 0);
    CHECK(llg_assoc_set_integral(&array, short_key, value));
    value.bits[0] = 17;
    CHECK(llg_assoc_set_integral(&array, wide_key, value));
    CHECK(llg_assoc_count(&array) == 1);
    CHECK(array.entries[0].integral_key.width < 65);
    expect_number(llg_assoc_get_integral(&array, short_key), 17);
    CHECK(llg_assoc_set_integral(&array, positive, value));
    CHECK(llg_assoc_count(&array) == 2);
    llg_assoc_set_default(&array, value);
    value.bits[0] = 99;
    expect_number(llg_assoc_get_integral(&array, absent), 17);
    llg_assoc_copy(&copy, &array);
    llg_assoc_copy(&array, &array);
    size_t retained = value_test_live();
    for (unsigned i = 0; i < 1000; ++i) {
        CHECK(llg_assoc_set_integral(&array, wide_key, value));
        llg_assoc_set_default(&array, value);
        llg_assoc_copy(&copy, &array);
        CHECK(value_test_live() == retained);
    }
    llg_assoc_destroy(&array);
    llg_assoc_destroy(&copy);
    sv4_destroy(&short_key);
    sv4_destroy(&wide_key);
    sv4_destroy(&positive);
    sv4_destroy(&absent);
    sv4_destroy(&value);
    CHECK(value_test_live() == 0);
}

static void check_recursive_owners(void) {
    const llg_value_desc_t packed_desc = {
        .kind = LLG_VALUE_PACKED, .packed_width = 65,
    };
    const llg_value_desc_t fixed_desc = {
        .kind = LLG_VALUE_FIXED_ARRAY, .element = &packed_desc, .item_count = 2,
    };
    llg_value_t value = {0};
    llg_value_default(&value, &fixed_desc);
    sv4_replace(&value.value.items[0].value.packed, sv4_from_u64(7, 65, 0));
    llg_value_copy(&value, &fixed_desc, &value);
    llg_value_copy(&value, &packed_desc, &value.value.items[0]);
    expect_number(sv4_clone(&value.value.packed), 7);
    llg_value_drop(&value);
    llg_queue_value_array_t queue;
    llg_queue_value_init(&queue, &packed_desc, UINT64_MAX);
    sv4_t source = sv4_from_u64(19, 65, 0);
    sv4_t index = sv4_zero(32, 0);
    size_t retained = value_test_live();
    for (unsigned i = 0; i < 1000; ++i) {
        llg_queue_value_push_back(&queue, source);
        llg_queue_value_push_front(&queue, source);
        llg_queue_value_copy(&queue, &queue);
        expect_number(llg_queue_value_get(&queue, index), 19);
        CHECK(llg_queue_value_delete_index(&queue, index));
        CHECK(llg_queue_value_delete_index(&queue, index));
        CHECK(value_test_live() == retained);
    }
    llg_queue_value_destroy(&queue);
    llg_assoc_value_t assoc;
    llg_assoc_value_init_integral(&assoc, &packed_desc, 32, 0, 0);
    retained = value_test_live();
    for (unsigned i = 0; i < 1000; ++i) {
        CHECK(llg_assoc_value_set_integral(&assoc, index, source));
        expect_number(llg_assoc_value_get_integral(&assoc, index), 19);
        llg_assoc_value_copy(&assoc, &assoc);
        CHECK(llg_assoc_value_delete_integral(&assoc, index));
        CHECK(value_test_live() == retained);
    }
    llg_assoc_value_destroy(&assoc);
    sv4_destroy(&index);
    sv4_destroy(&source);
    CHECK(value_test_live() == 0);
}

int main(void) {
    check_dynamic_array();
    check_pinned_queue();
    check_associative_owners();
    check_recursive_owners();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("container ownership and pinned references: OK");
    return 0;
}
