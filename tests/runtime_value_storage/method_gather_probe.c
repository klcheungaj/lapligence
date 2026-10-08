/* SIM-019 array-method result helpers: gathering by ordinal positions (with
 * out-of-range positions skipped and an aliased destination), hashed unique
 * positions over packed, string and real keys, and stable sorts by
 * precomputed packed and string keys, with tracked owner accounting. */
#include "llg_container.c"
#include "probe.h"
#include <math.h>
#include <string.h>

static const llg_value_desc_t text_desc = {LLG_VALUE_STRING, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t real_desc = {LLG_VALUE_REAL, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};

static void push_number(llg_queue_t* queue, uint64_t number) {
    sv4_t value = sv4_from_u64(number, 32, 1);
    llg_queue_push_back(queue, value);
    sv4_destroy(&value);
}

static void push_text(llg_queue_value_array_t* queue, const char* text) {
    llg_queue_value_push_back_string(queue, llg_string_bytes(text, strlen(text)));
}

static uint64_t number_at(const llg_queue_t* queue, uint64_t index) {
    sv4_t position = sv4_from_u64(index, 32, 1);
    sv4_t value = llg_queue_get(queue, position);
    sv4_destroy(&position);
    CHECK(!sv4_is_unknown(value));
    uint64_t number = sv4_to_u64(value);
    sv4_destroy(&value);
    return number;
}

static void expect_text_at(const llg_queue_value_array_t* queue, uint64_t index,
                           const char* text) {
    sv4_t position = sv4_from_u64(index, 32, 1);
    llg_string_t value = llg_queue_value_get_string(queue, position);
    sv4_destroy(&position);
    CHECK(value.len == strlen(text));
    CHECK(value.len == 0 || memcmp(value.data, text, value.len) == 0);
    llg_string_destroy(&value);
}

static void check_packed_gather_and_unique(void) {
    llg_queue_t source, positions, result;
    llg_queue_init(&source, 32, 1, 0, UINT64_MAX);
    llg_queue_init(&positions, 32, 1, 0, UINT64_MAX);
    llg_queue_init(&result, 32, 1, 0, UINT64_MAX);
    /* 3 1 3 X 1 7 */
    push_number(&source, 3);
    push_number(&source, 1);
    push_number(&source, 3);
    sv4_t unknown = sv4_x(32, 1);
    llg_queue_push_back(&source, unknown);
    push_number(&source, 1);
    push_number(&source, 7);

    llg_method_unique_positions(&positions, &source);
    /* First of each distinct key; X is a distinct key of its own. */
    CHECK(llg_queue_size(&positions) == 4);
    CHECK(number_at(&positions, 0) == 0);
    CHECK(number_at(&positions, 1) == 1);
    CHECK(number_at(&positions, 2) == 3);
    CHECK(number_at(&positions, 3) == 5);

    /* An out-of-range and an unknown position are skipped. */
    push_number(&positions, 99);
    llg_queue_push_back(&positions, unknown);
    llg_queue_gather(&result, &source, &positions, 0);
    CHECK(llg_queue_size(&result) == 4);
    CHECK(number_at(&result, 0) == 3);
    CHECK(number_at(&result, 3) == 7);
    llg_queue_gather(&result, &source, &positions, 1);
    CHECK(llg_queue_size(&result) == 4);
    CHECK(number_at(&result, 2) == 3);

    /* The destination may alias the source. */
    llg_queue_gather(&source, &source, &positions, 0);
    CHECK(llg_queue_size(&source) == 4);
    CHECK(number_at(&source, 1) == 1);

    /* Stable descending sort by keys 5 9 5 1: original 1 before 0 and 2. */
    llg_queue_t keys;
    llg_queue_init(&keys, 32, 1, 0, UINT64_MAX);
    llg_queue_delete(&source);
    for (uint64_t i = 0; i < 4; ++i) push_number(&source, 10 + i);
    push_number(&keys, 5);
    push_number(&keys, 9);
    push_number(&keys, 5);
    push_number(&keys, 1);
    llg_queue_sort_by_keys(&source, &keys, NULL, 1);
    CHECK(number_at(&source, 0) == 11);
    CHECK(number_at(&source, 1) == 10);
    CHECK(number_at(&source, 2) == 12);
    CHECK(number_at(&source, 3) == 13);
    /* A stale key count leaves the receiver unchanged. */
    push_number(&keys, 0);
    llg_queue_sort_by_keys(&source, &keys, NULL, 0);
    CHECK(number_at(&source, 0) == 11);

    sv4_destroy(&unknown);
    llg_queue_destroy(&keys);
    llg_queue_destroy(&source);
    llg_queue_destroy(&positions);
    llg_queue_destroy(&result);
    CHECK(value_test_live() == 0);
}

static void check_value_keys(void) {
    llg_queue_value_array_t words, keys, result;
    llg_queue_t positions;
    llg_queue_value_init(&words, &text_desc, UINT64_MAX);
    llg_queue_value_init(&keys, &text_desc, UINT64_MAX);
    llg_queue_value_init(&result, &text_desc, UINT64_MAX);
    llg_queue_init(&positions, 32, 1, 0, UINT64_MAX);
    const char* text[] = {"pear", "fig", "pear", "", "fig", "apple"};
    for (size_t i = 0; i < 6; ++i) {
        push_text(&words, text[i]);
        push_text(&keys, text[i]);
    }
    llg_method_unique_value_positions(&positions, &keys);
    CHECK(llg_queue_size(&positions) == 4);
    CHECK(number_at(&positions, 2) == 3);
    llg_queue_value_gather(&result, &words, &positions);
    CHECK(llg_queue_value_size(&result) == 4);
    expect_text_at(&result, 0, "pear");
    expect_text_at(&result, 2, "");
    expect_text_at(&result, 3, "apple");

    llg_queue_value_sort_by_keys(&words, NULL, &keys, 0);
    expect_text_at(&words, 0, "");
    expect_text_at(&words, 1, "apple");
    expect_text_at(&words, 2, "fig");
    expect_text_at(&words, 5, "pear");

    /* Real keys: NaN equals nothing, so each NaN is distinct; equal values
     * keep the first position. */
    llg_queue_value_array_t reals;
    llg_queue_value_init(&reals, &real_desc, UINT64_MAX);
    llg_queue_value_push_back_real(&reals, 2.5);
    llg_queue_value_push_back_real(&reals, NAN);
    llg_queue_value_push_back_real(&reals, 2.5);
    llg_queue_value_push_back_real(&reals, -1.0);
    llg_queue_value_push_back_real(&reals, NAN);
    llg_method_unique_value_positions(&positions, &reals);
    CHECK(llg_queue_size(&positions) == 4);
    CHECK(number_at(&positions, 0) == 0);
    CHECK(number_at(&positions, 1) == 1);
    CHECK(number_at(&positions, 2) == 3);
    CHECK(number_at(&positions, 3) == 4);

    llg_queue_value_destroy(&reals);
    llg_queue_value_destroy(&words);
    llg_queue_value_destroy(&keys);
    llg_queue_value_destroy(&result);
    llg_queue_destroy(&positions);
    CHECK(value_test_live() == 0);
}

int main(void) {
    check_packed_gather_and_unique();
    check_value_keys();
    puts("method gather probe passed");
    return 0;
}
