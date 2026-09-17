/* Mirrors owner-emitter event selection: the index descriptors are borrowed,
 * the selected handle is stable, and a null selection is an inert wait source. */
#include "llg_rt.c"
#include "probe.h"

static llg_event_object_t objects[4];
static llg_event_t handles[4];
static llg_event_t* const elements[] = {&handles[0], &handles[1], &handles[2], &handles[3]};
static const int32_t left[] = {2, 0};
static const int32_t right[] = {1, 1};
static unsigned valid_wakes;
static unsigned invalid_wakes;

static void select_indices(sv4_t* indices, uint64_t first, uint64_t second) {
    sv4_replace(&indices[0], sv4_from_u64(first, 32, 1));
    sv4_replace(&indices[1], sv4_from_u64(second, 32, 1));
}

static llg_event_t* select_event(const sv4_t* indices) {
    return llg_event_array_select(elements, 4, left, right, indices, 2);
}

static void valid_waiter(llg_proc_t* self) {
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* indices = llg_value_scope_values(scope);
    select_indices(indices, 1, 1);
    llg_event_t* address = select_event(indices);
    llg_value_scope_end(scope);  /* no index payload is retained by the wait */
    llg_event_t empty = {NULL};
    llg_expr_event_spec_t spec = {.kind = LLG_EV_ANY, .event = address ? address : &empty};
    llg_wait_expressions(&spec, 1);
    ++valid_wakes;
    llg_proc_done(self);
}

static void invalid_waiter(llg_proc_t* self) {
    llg_value_scope_t* scope = llg_value_scope_begin(2);
    sv4_t* indices = llg_value_scope_values(scope);
    sv4_replace(&indices[0], sv4_x(32, 1));
    sv4_replace(&indices[1], sv4_from_u64(1, 32, 1));
    llg_event_t* address = select_event(indices);
    llg_value_scope_end(scope);
    CHECK(address == NULL);
    llg_event_t empty = {NULL};
    llg_expr_event_spec_t spec = {.kind = LLG_EV_ANY, .event = address ? address : &empty};
    llg_wait_expressions(&spec, 1);
    ++invalid_wakes;
    llg_proc_done(self);
}

static void trigger_process(llg_proc_t* self) {
    (void)self;
    llg_wait_time(1);
    llg_event_trigger(&handles[3]);
    llg_wait_time(1);
    llg_rt_finish();
}

int main(int argc, char** argv) {
    int select_only = argc == 2 && strcmp(argv[1], "--select-only") == 0;
    for (unsigned cycle = 0; cycle < 8; ++cycle) {
        llg_rt_init();
        memset(objects, 0, sizeof(objects));
        for (size_t i = 0; i < 4; ++i) handles[i].object = &objects[i];
        llg_value_scope_t* scope = llg_value_scope_begin(2);
        sv4_t* indices = llg_value_scope_values(scope);
        select_indices(indices, 2, 0); CHECK(select_event(indices) == &handles[0]);
        select_indices(indices, 2, 1); CHECK(select_event(indices) == &handles[1]);
        select_indices(indices, 1, 0); CHECK(select_event(indices) == &handles[2]);
        select_indices(indices, 1, 1); CHECK(select_event(indices) == &handles[3]);
        select_indices(indices, 3, 1); CHECK(select_event(indices) == NULL);
        select_indices(indices, 1, UINT32_MAX); CHECK(select_event(indices) == NULL);
        sv4_replace(&indices[1], sv4_x(32, 1)); CHECK(select_event(indices) == NULL);
        llg_value_scope_end(scope);
        if (!select_only) {
            valid_wakes = invalid_wakes = 0;
            llg_spawn(valid_waiter, "valid-array-wait");
            llg_spawn(invalid_waiter, "invalid-array-wait");
            llg_spawn(trigger_process, "array-trigger");
            llg_rt_run();
            CHECK(valid_wakes == 1 && invalid_wakes == 0);
        }
        llg_rt_cleanup();
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
    puts("event array ownership: OK");
    return 0;
}
