/* SIM-006 recursive container values: whole record elements, in-place
 * element locators, identity-handle elements and their lifecycle hooks,
 * repeated deletion and (SIM-007) record container members moving between
 * nested slots and containers, with tracked packed-owner accounting. */
#include "llg_container.c"
#include "probe.h"
#include <string.h>

static const llg_value_desc_t wide_desc = {LLG_VALUE_PACKED, 0, 129, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t text_desc = {LLG_VALUE_STRING, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t real_desc = {LLG_VALUE_REAL, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_member_desc_t record_members[] = {{&text_desc}, {&wide_desc}, {&real_desc}};
static const llg_value_desc_t record_desc = {
    LLG_VALUE_AGGREGATE, 3, 0, 0, 0, 0, 3, NULL, record_members, 3};
static const llg_value_desc_t process_desc = {LLG_VALUE_PROCESS, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t event_desc = {LLG_VALUE_EVENT, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};

static int live_refs;
static int created_events;
static int event_objects[8];

static void count_retain(void* handle) { if (handle) ++live_refs; }
static void count_release(void* handle) { if (handle) --live_refs; }
static void* fresh_event(void) {
    CHECK(created_events < 8);
    return &event_objects[created_events++];
}
static const llg_value_handle_hooks_t hooks = {fresh_event, count_retain, count_release};

static void fill_record(llg_value_t* record, const char* text, uint64_t number, double real) {
    llg_native_value_init(record, &record_desc);
    llg_string_destroy(&record->value.items[0].value.string);
    record->value.items[0].value.string = llg_string_bytes(text, strlen(text));
    sv4_destroy(&record->value.items[1].value.packed);
    record->value.items[1].value.packed = sv4_from_u64(number, 129, 0);
    record->value.items[2].value.real = real;
}

static void expect_text(const llg_value_t* record, const char* text) {
    const llg_string_t* value = &record->value.items[0].value.string;
    CHECK(value->len == strlen(text));
    CHECK(value->len == 0 || memcmp(value->data, text, value->len) == 0);
}

static void check_record_queue(void) {
    llg_queue_value_array_t queue, copy;
    llg_queue_value_init(&queue, &record_desc, UINT64_MAX);
    llg_queue_value_init(&copy, &record_desc, UINT64_MAX);
    llg_value_t record = {0};
    fill_record(&record, "first", 7, 1.5);
    for (unsigned i = 0; i < 40; ++i) llg_queue_value_push_value(&queue, 1, &record);
    llg_native_value_destroy(&record);
    fill_record(&record, "front!", 9, 2.5);
    llg_queue_value_push_value(&queue, 0, &record);
    sv4_t one = sv4_from_u64(1, 32, 0);
    CHECK(llg_queue_value_insert_value(&queue, one, &record));
    CHECK(llg_queue_value_size(&queue) == 42);
    /* An in-place member write through the locator changes only that copy. */
    llg_value_t* element = llg_queue_value_element(&queue, &one, 1);
    CHECK(element);
    sv4_destroy(&element->value.items[1].value.packed);
    element->value.items[1].value.packed = sv4_from_u64(99, 129, 0);
    llg_queue_value_touch(&queue);
    llg_queue_value_copy(&copy, &queue);
    element->value.items[2].value.real = 8.0;
    llg_value_t read = {0};
    llg_native_value_init(&read, &record_desc);
    llg_value_element_read(&read, llg_queue_value_element(&copy, &one, 1));
    expect_number(sv4_clone(&read.value.items[1].value.packed), 99);
    CHECK(read.value.items[2].value.real == 2.5);
    /* A missing element reads the Table 7-1 default. */
    sv4_t far = sv4_from_u64(500, 32, 0);
    CHECK(!llg_queue_value_element(&queue, &far, 1));
    llg_value_element_read(&read, NULL);
    expect_text(&read, "");
    CHECK(read.value.items[2].value.real == 0.0);
    llg_queue_value_pop_value(&queue, 0, &read);
    expect_text(&read, "front!");
    CHECK(llg_queue_value_size(&queue) == 41);
    llg_queue_value_copy(&copy, &queue);
    size_t retained = value_test_live();
    for (unsigned i = 0; i < 200; ++i) {
        llg_queue_value_push_value(&queue, 1, &record);
        llg_queue_value_pop_value(&queue, 1, &read);
        llg_queue_value_copy(&copy, &queue);
        CHECK(value_test_live() == retained);
    }
    /* Repeated deletion and destruction after deletion are idempotent. */
    llg_queue_value_delete(&queue);
    llg_queue_value_delete(&queue);
    CHECK(llg_queue_value_size(&queue) == 0);
    llg_queue_value_destroy(&queue);
    llg_queue_value_destroy(&copy);
    llg_native_value_destroy(&read);
    llg_native_value_destroy(&record);
    sv4_destroy(&one);
    sv4_destroy(&far);
    CHECK(value_test_live() == 0);
}

static void check_assoc_records(void) {
    llg_assoc_value_t by_name, by_key;
    llg_assoc_value_init_string(&by_name, &record_desc);
    llg_assoc_value_init_integral(&by_key, &record_desc, 8, 0, 0);
    CHECK(!llg_assoc_value_element_string(&by_name, "k", 1, 0));
    llg_value_t* created = llg_assoc_value_element_string(&by_name, "k", 1, 1);
    CHECK(created);
    /* A created entry holds the element default: X for a 4-state member. */
    CHECK(sv4_is_unknown(created->value.items[1].value.packed));
    sv4_t unknown = sv4_x(8, 0);
    CHECK(!llg_assoc_value_element_integral(&by_key, &unknown, 1, 1));
    sv4_t key = sv4_from_u64(3, 8, 0);
    llg_value_t record = {0};
    fill_record(&record, "v", 5, 0.5);
    CHECK(!llg_assoc_value_set_element_integral(&by_key, &unknown, 1, &record));
    CHECK(llg_assoc_value_set_element_integral(&by_key, &key, 1, &record));
    CHECK(llg_assoc_value_element_integral(&by_key, &key, 1, 0));
    llg_assoc_value_destroy(&by_name);
    llg_assoc_value_destroy(&by_key);
    llg_native_value_destroy(&record);
    sv4_destroy(&unknown);
    sv4_destroy(&key);
    CHECK(value_test_live() == 0);
}

static void check_identity_handles(void) {
    llg_value_set_handle_hooks(&hooks);
    llg_queue_value_array_t processes, copy;
    llg_queue_value_init(&processes, &process_desc, UINT64_MAX);
    llg_queue_value_init(&copy, &process_desc, UINT64_MAX);
    int a = 0, b = 0;
    llg_queue_value_push_back_chandle(&processes, &a);
    llg_queue_value_push_back_chandle(&processes, &b);
    llg_queue_value_push_back_chandle(&processes, NULL);
    CHECK(live_refs == 2);
    llg_queue_value_copy(&copy, &processes);
    CHECK(live_refs == 4);
    llg_queue_value_method(&processes, LLG_CONTAINER_METHOD_REVERSE);
    CHECK(live_refs == 4);
    sv4_t zero = sv4_zero(32, 0);
    CHECK(llg_queue_value_get_chandle(&processes, zero) == NULL);
    void* popped = NULL;
    llg_queue_value_pop_process_to(&popped, &processes, 1);
    CHECK(popped == &a);
    CHECK(live_refs == 4); /* the reference moved into `popped` */
    count_release(popped);
    llg_queue_value_delete(&processes);
    llg_queue_value_delete(&processes);
    CHECK(live_refs == 2);
    llg_queue_value_destroy(&processes);
    llg_queue_value_destroy(&copy);
    CHECK(live_refs == 0);

    /* Associative writes and defaults keep exactly one reference per copy
     * (SIM-015: the integral-key and default setters dropped no source). */
    llg_assoc_value_t by_key;
    llg_assoc_value_init_integral(&by_key, &process_desc, 32, 0, 0);
    sv4_t three = sv4_from_u64(3, 32, 0);
    llg_assoc_value_set_integral_chandle(&by_key, three, &a);
    CHECK(live_refs == 1);
    llg_assoc_value_set_integral_chandle(&by_key, three, &b);
    CHECK(live_refs == 1);
    llg_assoc_value_set_default_chandle(&by_key, &a);
    CHECK(live_refs == 2);
    llg_assoc_value_destroy(&by_key);
    CHECK(live_refs == 0);
    sv4_destroy(&three);

    /* New event elements refer to new events; copies share them. */
    llg_dyn_value_array_t events, events_copy;
    llg_dyn_value_init(&events, &event_desc);
    llg_dyn_value_init(&events_copy, &event_desc);
    sv4_t two = sv4_from_u64(2, 32, 0);
    llg_dyn_value_new(&events, two, NULL);
    CHECK(created_events == 2);
    llg_dyn_value_copy(&events_copy, &events);
    CHECK(created_events == 2);
    sv4_t one = sv4_from_u64(1, 32, 0);
    CHECK(llg_dyn_value_get_chandle(&events_copy, one) == &event_objects[1]);
    llg_dyn_value_destroy(&events);
    llg_dyn_value_destroy(&events_copy);
    sv4_destroy(&zero);
    sv4_destroy(&one);
    sv4_destroy(&two);
    CHECK(value_test_live() == 0);
}

/* SIM-007: a record's queue/dynamic-array member moves between its nested
 * slot in a value and a standalone container, in both directions. */
static const llg_value_desc_t int_desc = {LLG_VALUE_PACKED, 0, 32, 1, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t ints_desc = {LLG_VALUE_CONTAINER, 11, 0, 0, 0, 0, 0, &int_desc, NULL, 0};
static const llg_value_desc_t texts_desc = {LLG_VALUE_CONTAINER, 12, 0, 0, 0, 0, 0, &text_desc, NULL, 0};
static const llg_value_member_desc_t holder_members[] = {{&text_desc}, {&ints_desc}, {&texts_desc}};
static const llg_value_desc_t holder_desc = {
    LLG_VALUE_AGGREGATE, 10, 0, 0, 0, 0, 3, NULL, holder_members, 3};

static int64_t packed_item(const llg_dyn_value_array_t* nested, size_t index) {
    int64_t value = 0;
    CHECK(sv4_to_index_i64(nested->data[index].value.packed, &value));
    return value;
}

static void check_record_container_members(void) {
    llg_value_t holder = {0};
    llg_native_value_init(&holder, &holder_desc);
    llg_value_t* ints = &holder.value.items[1];
    llg_value_t* texts = &holder.value.items[2];
    CHECK(ints->value.container == NULL && llg_value_container_size(ints) == 0);

    llg_queue_t queue;
    llg_queue_init(&queue, 32, 1, 0, UINT64_MAX);
    for (uint64_t i = 1; i <= 3; ++i) {
        sv4_t value = sv4_from_u64(i * 10, 32, 1);
        llg_queue_push_back(&queue, value);
        sv4_destroy(&value);
    }
    llg_value_item_from_queue(ints, &queue);
    CHECK(llg_value_container_size(ints) == 3);
    CHECK(packed_item(ints->value.container, 0) == 10);
    CHECK(packed_item(ints->value.container, 2) == 30);

    llg_dyn_array_t dyn;
    llg_dyn_init(&dyn, 32, 1, 0);
    llg_value_item_to_dyn(&dyn, ints);
    CHECK(llg_dyn_size(&dyn) == 3);
    llg_queue_t bounded;
    llg_queue_init(&bounded, 32, 1, 0, 2);
    llg_value_item_to_queue(&bounded, ints);
    CHECK(llg_queue_size(&bounded) == 2);

    /* A dynamic array source replaces the slot; an empty one leaves null. */
    sv4_t zero = sv4_from_u64(0, 32, 0);
    sv4_t seven = sv4_from_u64(7, 32, 1);
    CHECK(llg_dyn_set(&dyn, zero, seven));
    llg_value_item_from_dyn(ints, &dyn);
    CHECK(packed_item(ints->value.container, 0) == 7);
    llg_queue_delete(&queue);
    llg_value_item_from_queue(ints, &queue);
    CHECK(ints->value.container == NULL);
    llg_value_item_to_dyn(&dyn, ints);
    CHECK(llg_dyn_size(&dyn) == 0);

    llg_queue_value_array_t words, words_copy;
    llg_queue_value_init(&words, &text_desc, UINT64_MAX);
    llg_queue_value_init(&words_copy, &text_desc, 1);
    llg_value_t word = {0};
    llg_native_value_init(&word, &text_desc);
    word.value.string = llg_string_bytes("ab", 2);
    llg_queue_value_push_value(&words, 1, &word);
    llg_string_destroy(&word.value.string);
    word.value.string = llg_string_bytes("cde", 3);
    llg_queue_value_push_value(&words, 1, &word);
    llg_native_value_destroy(&word);
    llg_value_item_from_queue_value(texts, &words);
    CHECK(llg_value_container_size(texts) == 2);
    CHECK(texts->value.container->data[1].value.string.len == 3);
    llg_dyn_value_array_t words_dyn;
    llg_dyn_value_init(&words_dyn, &text_desc);
    llg_value_item_to_dyn_value(&words_dyn, texts);
    CHECK(llg_dyn_value_size(&words_dyn) == 2);
    llg_value_item_to_queue_value(&words_copy, texts);
    CHECK(llg_queue_value_size(&words_copy) == 1);
    llg_value_item_from_dyn_value(texts, &words_dyn);
    CHECK(llg_value_container_size(texts) == 2);

    /* Copies of the holder own their nested members. */
    llg_value_t copy = {0};
    llg_native_value_init(&copy, &holder_desc);
    llg_native_value_copy(&copy, &holder);
    llg_queue_value_delete(&words);
    llg_value_item_from_queue_value(texts, &words);
    CHECK(texts->value.container == NULL);
    CHECK(llg_value_container_size(&copy.value.items[2]) == 2);

    llg_native_value_destroy(&copy);
    llg_native_value_destroy(&holder);
    llg_queue_destroy(&queue);
    llg_queue_destroy(&bounded);
    llg_dyn_destroy(&dyn);
    llg_queue_value_destroy(&words);
    llg_queue_value_destroy(&words_copy);
    llg_dyn_value_destroy(&words_dyn);
    sv4_destroy(&zero);
    sv4_destroy(&seven);
    CHECK(value_test_live() == 0);
}

int main(void) {
    check_record_queue();
    check_assoc_records();
    check_identity_handles();
    check_record_container_members();
    puts("container value probe passed");
    return 0;
}
