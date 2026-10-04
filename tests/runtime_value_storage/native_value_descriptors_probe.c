/* Descriptor-backed native values (SIM-003): structural validation, storage
 * identity, independent deep copies, repeated construction, transactional
 * copies under item-allocation failure, borrowed chandles and handle tracing. */
#include <stddef.h>
#include <stdlib.h>

static size_t items_fail_countdown;
static size_t items_live;
static void* probe_items_malloc(size_t bytes) {
    if (items_fail_countdown && --items_fail_countdown == 0) return NULL;
    ++items_live;
    return malloc(bytes);
}
#define LLG_VALUE_ITEMS_MALLOC probe_items_malloc
#include "llg_container.c"
#include "probe.h"

/* struct { logic [69:0] p; string s; } */
static const llg_value_desc_t packed_desc = {LLG_VALUE_PACKED, 0, 70, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t bit_desc = {LLG_VALUE_PACKED, 0, 8, 0, 1, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t string_desc = {LLG_VALUE_STRING, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t real_desc = {LLG_VALUE_REAL, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t chandle_desc = {LLG_VALUE_CHANDLE, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t opaque_desc = {LLG_VALUE_OPAQUE, 9, 0, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_desc_t event_desc = {LLG_VALUE_EVENT, 0, 0, 0, 0, 0, 0, NULL, NULL, 0};
static const llg_value_member_desc_t inner_members[] = {{&packed_desc}, {&string_desc}};
static const llg_value_desc_t inner_desc = {LLG_VALUE_AGGREGATE, 2, 0, 0, 0, 0, 2, NULL, inner_members, 2};
static const llg_value_desc_t strings_desc = {LLG_VALUE_FIXED_ARRAY, 0, 0, 0, 0, 0, 3, &string_desc, NULL, 0};
/* struct { string s; real r; chandle h; inner i; string a[3]; bit [7:0] b; } */
static const llg_value_member_desc_t outer_members[] = {
    {&string_desc}, {&real_desc}, {&chandle_desc}, {&inner_desc}, {&strings_desc}, {&bit_desc}};
static const llg_value_desc_t outer_desc = {LLG_VALUE_AGGREGATE, 1, 0, 0, 0, 0, 6, NULL, outer_members, 6};

static void set_string(llg_string_t* target, const char* text) {
    llg_string_destroy(target);
    *target = llg_string_bytes(text, strlen(text));
}

static int string_is(const llg_string_t* value, const char* text) {
    size_t length = strlen(text);
    return value->len == length && (!length || memcmp(value->data, text, length) == 0);
}

static void fill(llg_value_t* value, const char* text, void* handle) {
    llg_value_t* items = value->value.items;
    set_string(&items[0].value.string, text);
    items[1].value.real = 1.5;
    items[2].value.handle = handle;
    sv4_replace(&items[3].value.items[0].value.packed, sv4_from_u64(42, 70, 0));
    set_string(&items[3].value.items[1].value.string, "inner");
    set_string(&items[4].value.items[2].value.string, "last");
}

static void check_validation(void) {
    CHECK(llg_value_desc_valid(&outer_desc));
    CHECK(llg_value_desc_valid(&strings_desc));
    CHECK(!llg_value_desc_valid(NULL));

    llg_value_desc_t bad = packed_desc;
    bad.packed_width = 0;
    CHECK(!llg_value_desc_valid(&bad));
    bad.packed_width = LLG_SUPPORTED_WIDTH_LIMIT;
    CHECK(!llg_value_desc_valid(&bad));
    bad = string_desc;
    bad.kind = 42;
    CHECK(!llg_value_desc_valid(&bad));

    llg_value_desc_t aggregate = inner_desc;
    aggregate.type_id = 0; /* nominal identity is mandatory */
    CHECK(!llg_value_desc_valid(&aggregate));
    aggregate = inner_desc;
    aggregate.item_count = 3; /* disagrees with member_count */
    CHECK(!llg_value_desc_valid(&aggregate));

    llg_value_desc_t huge = strings_desc;
    huge.item_count = SIZE_MAX; /* allocation size would overflow */
    CHECK(!llg_value_desc_valid(&huge));
    huge.item_count = 0;
    CHECK(!llg_value_desc_valid(&huge));

    /* A self-referential aggregate and an over-deep chain are both rejected
     * by bounded descent instead of unbounded recursion. */
    llg_value_member_desc_t cyclic_member[1];
    llg_value_desc_t cyclic = {LLG_VALUE_AGGREGATE, 5, 0, 0, 0, 0, 1, NULL, cyclic_member, 1};
    cyclic_member[0].value = &cyclic;
    CHECK(!llg_value_desc_valid(&cyclic));
    llg_value_desc_t chain[LLG_VALUE_DESC_MAX_DEPTH + 1];
    for (size_t i = 0; i < LLG_VALUE_DESC_MAX_DEPTH + 1; ++i) {
        chain[i] = strings_desc;
        chain[i].element = i ? &chain[i - 1] : &string_desc;
    }
    CHECK(llg_value_desc_valid(&chain[LLG_VALUE_DESC_MAX_DEPTH - 2]));
    CHECK(!llg_value_desc_valid(&chain[LLG_VALUE_DESC_MAX_DEPTH]));
}

static void check_policies(void) {
    CHECK(llg_value_desc_copy_policy(&packed_desc) == LLG_VALUE_COPY_DEEP);
    CHECK(llg_value_desc_copy_policy(&string_desc) == LLG_VALUE_COPY_DEEP);
    CHECK(llg_value_desc_copy_policy(&outer_desc) == LLG_VALUE_COPY_DEEP);
    CHECK(llg_value_desc_copy_policy(&strings_desc) == LLG_VALUE_COPY_DEEP);
    CHECK(llg_value_desc_copy_policy(&opaque_desc) == LLG_VALUE_COPY_IDENTITY);
    CHECK(llg_value_desc_copy_policy(&event_desc) == LLG_VALUE_COPY_IDENTITY);
    CHECK(llg_value_desc_copy_policy(&chandle_desc) == LLG_VALUE_COPY_BORROWED);
    CHECK(llg_value_desc_copy_policy(NULL) == -1);
}

static void check_defaults_and_copies(void) {
    llg_value_t first = {0};
    llg_native_value_init(&first, &outer_desc);
    llg_value_t* items = first.value.items;
    CHECK(items[0].value.string.len == 0 && items[1].value.real == 0.0);
    CHECK(items[2].value.handle == NULL);
    CHECK(sv4_is_unknown(items[3].value.items[0].value.packed));
    expect_number(sv4_clone(&items[5].value.packed), 0); /* two-state default */

    /* A borrowed foreign pointer: copies share it, destruction never frees it. */
    char* foreign = malloc(16);
    CHECK(foreign != NULL);
    strcpy(foreign, "foreign");
    fill(&first, "outer", foreign);

    llg_value_t second = {0};
    llg_native_value_init(&second, &outer_desc);
    llg_native_value_copy(&second, &first);
    set_string(&first.value.items[0].value.string, "changed");
    set_string(&first.value.items[3].value.items[1].value.string, "changed");
    sv4_replace(&first.value.items[3].value.items[0].value.packed, sv4_from_u64(1, 70, 0));
    llg_value_t* copied = second.value.items;
    CHECK(string_is(&copied[0].value.string, "outer"));
    CHECK(copied[1].value.real == 1.5);
    CHECK(copied[2].value.handle == foreign);
    expect_number(sv4_clone(&copied[3].value.items[0].value.packed), 42);
    CHECK(string_is(&copied[3].value.items[1].value.string, "inner"));
    CHECK(string_is(&copied[4].value.items[2].value.string, "last"));

    llg_native_value_copy(&second, &second); /* self copy keeps the value */
    CHECK(string_is(&second.value.items[0].value.string, "outer"));

    llg_native_value_destroy(&first);
    llg_native_value_destroy(&second);
    CHECK(first.desc == NULL && second.desc == NULL);
    llg_native_value_destroy(&first); /* destroying an empty value is safe */
    CHECK(strcmp(foreign, "foreign") == 0);
    free(foreign);
}

static void check_repeated_construction(void) {
    size_t packed_live = value_test_live();
    size_t items_before = items_live;
    llg_value_t source = {0};
    llg_native_value_init(&source, &outer_desc);
    fill(&source, "repeat", NULL);
    for (unsigned i = 0; i < 1000; ++i) {
        llg_value_t value = {0};
        llg_native_value_init(&value, &outer_desc);
        llg_native_value_copy(&value, &source);
        CHECK(string_is(&value.value.items[3].value.items[1].value.string, "inner"));
        llg_native_value_destroy(&value);
    }
    llg_native_value_destroy(&source);
    CHECK(value_test_live() == packed_live);
    CHECK(items_live > items_before);
}

static void check_failed_copy_is_atomic(void) {
    llg_value_t target = {0};
    llg_value_t source = {0};
    llg_native_value_init(&target, &outer_desc);
    llg_native_value_init(&source, &outer_desc);
    fill(&target, "kept", NULL);
    fill(&source, "replacement", NULL);
    size_t packed_live = value_test_live();
    /* Fail each item allocation of the replacement in turn: the outer items,
     * the nested record and the fixed string array. */
    for (size_t failure = 1; failure <= 3; ++failure) {
        items_fail_countdown = failure;
        CHECK(!llg_native_value_try_copy(&target, &source));
        items_fail_countdown = 0;
        CHECK(string_is(&target.value.items[0].value.string, "kept"));
        expect_number(sv4_clone(&target.value.items[3].value.items[0].value.packed), 42);
        CHECK(value_test_live() == packed_live);
    }
    CHECK(llg_native_value_try_copy(&target, &source));
    CHECK(string_is(&target.value.items[0].value.string, "replacement"));
    llg_native_value_destroy(&target);
    llg_native_value_destroy(&source);
}

typedef struct {
    size_t visits;
    void* seen[4];
} trace_log_t;

static void record_handle(void* const* slot, const llg_value_desc_t* desc, void* context) {
    trace_log_t* log = context;
    CHECK(llg_value_desc_copy_policy(desc) == LLG_VALUE_COPY_IDENTITY);
    CHECK(log->visits < 4);
    log->seen[log->visits++] = *slot;
}

static void check_trace(void) {
    static const llg_value_desc_t handles_desc = {LLG_VALUE_FIXED_ARRAY, 0, 0, 0, 0, 0, 2, &opaque_desc, NULL, 0};
    static const llg_value_desc_t list_desc = {LLG_VALUE_CONTAINER, 0, 0, 0, 0, 0, 0, &opaque_desc, NULL, 0};
    static const llg_value_member_desc_t members[] = {
        {&chandle_desc}, {&handles_desc}, {&event_desc}, {&list_desc}};
    static const llg_value_desc_t record = {LLG_VALUE_AGGREGATE, 3, 0, 0, 0, 0, 4, NULL, members, 4};
    CHECK(llg_value_desc_valid(&record));
    int objects[5];
    llg_value_t value = {0};
    llg_native_value_init(&value, &record);
    value.value.items[0].value.handle = &objects[0]; /* borrowed: not traced */
    value.value.items[1].value.items[1].value.handle = &objects[1];
    value.value.items[2].value.handle = &objects[2];
    llg_dyn_value_array_t* list = malloc(sizeof(*list));
    CHECK(list != NULL);
    llg_dyn_value_init(list, &opaque_desc);
    sv4_t size = sv4_from_u64(2, 32, 0);
    llg_dyn_value_new(list, size, NULL);
    sv4_destroy(&size);
    list->data[0].value.handle = &objects[3];
    value.value.items[3].value.container = list;
    trace_log_t log = {0};
    llg_value_trace(&value, record_handle, &log);
    CHECK(log.visits == 3);
    CHECK(log.seen[0] == &objects[1] && log.seen[1] == &objects[2] && log.seen[2] == &objects[3]);
    llg_native_value_destroy(&value);
}

static void check_roots(void) {
    static const llg_value_member_desc_t members[] = {{&opaque_desc}, {&string_desc}};
    static const llg_value_desc_t record = {LLG_VALUE_AGGREGATE, 4, 0, 0, 0, 0, 2, NULL, members, 2};
    int objects[3];
    llg_native_root_t roots[3];
    CHECK(llg_native_roots_count() == 0);
    for (size_t i = 0; i < 3; ++i) {
        llg_native_root_init(&roots[i], &record);
        roots[i].value.value.items[0].value.handle = &objects[i];
        set_string(&roots[i].value.value.items[1].value.string, "rooted");
    }
    CHECK(llg_native_roots_count() == 3);
    trace_log_t log = {0};
    llg_native_roots_trace(record_handle, &log);
    CHECK(log.visits == 3);
    /* Unlinking the middle root keeps the others enumerable. */
    llg_native_root_destroy(&roots[1]);
    llg_native_root_destroy(&roots[1]);
    CHECK(llg_native_roots_count() == 2);
    trace_log_t after = {0};
    llg_native_roots_trace(record_handle, &after);
    CHECK(after.visits == 2);
    CHECK((after.seen[0] == &objects[0] || after.seen[0] == &objects[2]) &&
          after.seen[0] != after.seen[1]);
    llg_native_root_destroy(&roots[2]);
    llg_native_root_destroy(&roots[0]);
    CHECK(llg_native_roots_count() == 0);
}

int main(void) {
    check_validation();
    check_policies();
    check_defaults_and_copies();
    check_repeated_construction();
    check_failed_copy_is_atomic();
    check_trace();
    check_roots();
    CHECK(value_test_live() == 0);
    return 0;
}
