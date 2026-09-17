/* Native-value scopes and input snapshots must survive nonlocal termination.
 * This is a C runtime probe, not evidence that the Rust emitter was executed. */
#include "llg_rt.c"
#include "probe.h"

static unsigned strings_destroyed;
static unsigned handles_destroyed;
static unsigned evaluations;
static unsigned mode;
static sv4_t watched = SV4_EMPTY;
static sv4_t payload = SV4_EMPTY;
static llg_queue_t queue;
static llg_queue_value_array_t value_queue;
static llg_assoc_value_t value_assoc;
static sv4_t index_value = SV4_EMPTY;
static const llg_value_desc_t packed_desc = {
    .kind = LLG_VALUE_PACKED, .packed_width = 129, .packed_two_state = 1
};
static llg_string_t output_string;

static void string_drop(void* pointer) {
    llg_string_destroy((llg_string_t*)pointer);
    ++strings_destroyed;
}
static void handle_drop(void* pointer) {
    llg_process_release(*(llg_process_handle_t**)pointer);
    ++handles_destroyed;
}
static llg_string_t* new_string(const char* text) {
    llg_value_scope_t* scope = llg_value_scope_begin_object(sizeof(llg_string_t), string_drop);
    llg_string_t* value = (llg_string_t*)llg_value_scope_object(scope);
    *value = llg_string_bytes(text, strlen(text));
    return value;
}
static void root_scopes(void) {
    for (unsigned repeat = 0; repeat < 16; ++repeat) {
        llg_rt_init();
        unsigned before = strings_destroyed;
        llg_value_scope_t* root = llg_value_scope_mark();
        llg_string_t* first = new_string("abc");
        llg_value_scope_t* mark = llg_value_scope_mark();
        llg_string_t* second = new_string("");
        *second = llg_string_clone(first);
        second->data[0] = 'x';
        CHECK(first->data[0] == 'a');
        llg_string_t transferred = llg_string_take(second);
        CHECK(second->data == NULL && second->len == 0);
        llg_value_scopes_end_since(mark);
        CHECK(strings_destroyed == before + 1);
        CHECK(transferred.len == 3 && transferred.data[0] == 'x');
        llg_string_destroy(&transferred);
        llg_value_scopes_end_since(root);
        CHECK(strings_destroyed == before + 2);
        CHECK(all_value_scopes == NULL && root_value_scopes == NULL);
        llg_rt_cleanup();
        CHECK(value_test_live() == 0);
    }
}
static void evaluator(sv4_t* result, void* context) {
    (void)context;
    ++evaluations;
    if (evaluations == 2) llg_rt_finish();
    sv4_copy(result, &watched);
}
static void waiter(llg_proc_t* self) {
    sv4_t* reads[] = {&watched};
    llg_expr_event_spec_t spec = {0};
    spec.eval = evaluator;
    spec.reads = reads;
    spec.n_reads = 1;
    spec.kind = LLG_EV_ANY;
    llg_wait_expressions(&spec, 1);
    llg_proc_done(self);
}
static void writer(llg_proc_t* self) {
    (void)new_string("held across yield and termination");
    llg_value_scope_t* handle_scope = llg_value_scope_begin_object(
        sizeof(llg_process_handle_t*), handle_drop);
    llg_process_handle_t** handle = (llg_process_handle_t**)llg_value_scope_object(handle_scope);
    llg_process_assign(handle, llg_process_self());
    llg_wait_time(1);
    llg_ref_t target = { .base = &watched, .width = 129, .kind = LLG_REF_WHOLE };
    if (mode == 0) {
        llg_ref_write(&target, payload);
    } else if (mode == 1) {
        llg_file_input_target_t input = { .kind = LLG_FILE_INPUT_PACKED, .packed = &target };
        (void)llg_string_scanf("7", 1, "%d", &input, 1);
    } else if (mode == 2 || mode == 3) {
        uint32_t fd = llg_file_open(llg_string_bytes("native-input.tmp", 16),
                                    llg_string_bytes("r", 1), 1);
        if (mode == 2) (void)llg_file_gets_packed(fd, &target);
        else (void)llg_file_read_packed(fd, &target);
    } else if (mode == 4) {
        llg_value_scope_t* value_scope = llg_value_scope_begin(1);
        sv4_t* result = llg_value_scope_values(value_scope);
        llg_queue_pop_front_into(&queue, result);
        llg_value_scope_end(value_scope);
    } else if (mode == 5) {
        (void)llg_value_plusargs_string("text=%s", &output_string);
    } else if (mode == 6) {
        llg_file_input_target_t input = { .kind = LLG_FILE_INPUT_STRING, .string = &output_string };
        (void)llg_string_scanf("hello", 5, "%s", &input, 1);
    } else if (mode == 7) {
        llg_queue_value_push_back(&value_queue, payload);
    } else if (mode == 8) {
        (void)llg_queue_value_set(&value_queue, index_value, payload);
    } else if (mode == 9) {
        (void)llg_queue_value_set_nested(&value_queue, &index_value, 1, payload);
    } else if (mode == 10) {
        (void)llg_queue_value_insert(&value_queue, index_value, payload);
    } else if (mode == 11) {
        (void)llg_assoc_value_set_integral(&value_assoc, index_value, payload);
    } else if (mode == 12) {
        (void)llg_assoc_value_delete_integral(&value_assoc, index_value);
    } else if (mode == 13) {
        llg_assoc_value_set_default(&value_assoc, payload);
    } else {
        llg_queue_value_push_front(&value_queue, payload);
    }
    /* Every operation above notifies the waiter and terminates this coroutine. */
    CHECK(0);
    llg_proc_done(self);
}
static void nonlocal_scopes(void) {
    FILE* file = fopen("native-input.tmp", "wb");
    CHECK(file != NULL);
    CHECK(fwrite("7\n", 1, 2, file) == 2);
    CHECK(fclose(file) == 0);
    for (mode = 0; mode < 15; ++mode) {
        for (unsigned repeat = 0; repeat < 8; ++repeat) {
            char* args[] = {"native-probe", "+text=value"};
            llg_rt_init_with_args(2, args);
            evaluations = 0;
            unsigned before_strings = strings_destroyed;
            unsigned before_handles = handles_destroyed;
            sv4_replace(&watched, sv4_zero(129, 0));
            sv4_replace(&payload, sv4_from_u64(1, 129, 0));
            llg_queue_init(&queue, 129, 0, 0, UINT64_MAX);
            llg_queue_push_back(&queue, payload);
            sv4_replace(&index_value, sv4_zero(32, 0));
            llg_queue_value_init(&value_queue, &packed_desc, UINT64_MAX);
            llg_queue_value_push_back(&value_queue, watched);
            value_queue.contents_dependency = &watched;
            value_queue.notify = llg_dependency_notify;
            llg_assoc_value_init_integral(&value_assoc, &packed_desc, 32, 0, 0);
            (void)llg_assoc_value_set_integral(&value_assoc, index_value, watched);
            value_assoc.contents_dependency = &watched;
            value_assoc.notify = llg_dependency_notify;
            queue.contents_dependency = &watched;
            queue.notify = llg_dependency_notify;
            output_string = (llg_string_t){0};
            output_string.notify = llg_dependency_changed;
            output_string.dependency = &watched;
            llg_spawn(waiter, "native-owner-waiter");
            llg_spawn(writer, "native-owner-writer");
            llg_rt_run();
            CHECK(evaluations == 2);
            llg_rt_cleanup();
            llg_queue_destroy(&queue);
            llg_queue_value_destroy(&value_queue);
            llg_assoc_value_destroy(&value_assoc);
            sv4_destroy(&index_value);
            llg_string_destroy(&output_string);
            sv4_destroy(&watched); sv4_destroy(&payload);
            CHECK(strings_destroyed == before_strings + 1);
            CHECK(handles_destroyed == before_handles + 1);
            if (value_test_live() != 0) fprintf(stderr, "mode=%u repeat=%u\n", mode, repeat);
            CHECK(value_test_live() == 0 && value_test_bytes() == 0);
            CHECK(all_value_scopes == NULL && root_value_scopes == NULL);
        }
    }
    CHECK(remove("native-input.tmp") == 0);
}
int main(int argc, char** argv) {
    root_scopes();
    if (argc < 2 || strcmp(argv[1], "--scopes-only") != 0) nonlocal_scopes();
    puts("native owner cleanup: OK");
    return 0;
}
