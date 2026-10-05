/* SIM-006 recursive container values: whole record elements, in-place
 * element locators, identity-handle elements and their lifecycle hooks,
 * and repeated deletion, with tracked packed-owner accounting. */
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

int main(void) {
    check_record_queue();
    check_assoc_records();
    check_identity_handles();
    puts("container value probe passed");
    return 0;
}
