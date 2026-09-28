/* Runtime-only checks. These do not invoke the Rust emitter. */
#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static sv4_t watched = SV4_EMPTY;
static sv4_t payload = SV4_EMPTY;
static sv4_t zero = SV4_EMPTY;
static sv4_t* destination;
static llg_mailbox_t* mailbox;
static llg_process_handle_t* receiver_handle;
static llg_dyn_array_t dynamic_array;
static llg_queue_t queue;
static unsigned mode;
static unsigned evaluations;

static void root_reference_scopes(void) {
    for (unsigned round = 0; round < 16; ++round) {
        llg_rt_init();
        llg_queue_init(&queue, 129, 0, 0, UINT64_MAX);
        sv4_replace(&payload, sv4_from_u64(7, 129, 0));
        sv4_replace(&zero, sv4_zero(64, 0));
        llg_queue_push_back(&queue, payload);
        llg_value_scope_t* mark = llg_value_scope_mark();
        llg_ref_scope_begin_owned();
        llg_ref_t* reference = llg_ref_queue(&queue, 0);
        expect_number(llg_ref_read(reference), 7);
        llg_queue_delete(&queue);
        expect_number(llg_ref_read(reference), 7);
        llg_ref_write(reference, payload);
        CHECK(llg_queue_size(&queue) == 0);
        llg_value_scopes_end_since(mark);
        CHECK(root_reference_top == NULL);
        llg_queue_destroy(&queue);
        sv4_destroy(&payload); sv4_destroy(&zero);
        llg_rt_cleanup();
        CHECK(value_test_live() == 0 && all_value_scopes == NULL);
    }
}

static void evaluator(sv4_t* out, void* context) {
    (void)context;
    ++evaluations;
    if (evaluations == 2) {
        if (mode < 4) {
            CHECK(llg_mailbox_num(mailbox) == (mode % 2 ? 1u : 0u));
            /* Reentrant insertion sees the already committed consume/peek. */
            CHECK(llg_mailbox_try_put_value(
                mailbox, llg_mailbox_value_packed(payload, 129, 0, 0)));
            CHECK(llg_mailbox_num(mailbox) == (mode % 2 ? 2u : 1u));
            llg_rt_finish();
            if (llg_rt_exiting()) return;
        }
        if (mode == 4) {
            llg_process_kill(llg_current(), receiver_handle);
            /* The publisher pins this descriptor even after its owner dies. */
            CHECK(value_scope_index_find(destination) != NULL);
            CHECK(sv4_to_u64(*destination) == 7);
        } else {
            llg_rt_finish();
            if (llg_rt_exiting()) return;
        }
    }
    sv4_copy(out, destination);
}
static llg_co_arm_t arm_observer(llg_proc_t* self) {
    sv4_t* reads[] = {destination};
    llg_expr_event_spec_t event = {0};
    event.eval = evaluator; event.reads = reads; event.n_reads = 1;
    event.kind = LLG_EV_ANY;
    return llg_arm_expressions(self, &event, 1);
}
LLG_PROBE_SIMPLE_PROCESS(observer, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, arm_observer(self));
    CHECK(mode == 4);
    LLG_PROBE_DONE();
}
LLG_PROBE_SIMPLE_PROCESS(receiver, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    if (mode == 4) {
        destination = llg_value_scope_values(llg_value_scope_begin(1));
        sv4_replace(destination, sv4_zero(129, 0));
        llg_process_assign(&receiver_handle, llg_process_self(self));
    }
    LLG_PROBE_AWAIT(
        1, llg_arm_mailbox_get_value(
               self, mailbox,
               llg_mailbox_target_packed(destination, 129, 0, 0),
               mode % 2 == 1));
    CHECK(0); /* finish or kill must prevent this continuation. */
    LLG_PROBE_DONE();
}
LLG_PROBE_SIMPLE_PROCESS(publisher, 2) {
    LLG_PROBE_SIMPLE_BEGIN(2);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    if (mode < 2) {
        (void)llg_mailbox_try_get_value(mailbox, llg_mailbox_target_packed(destination, 129, 0, 0), mode % 2 == 1);
    } else if (mode < 5) {
        LLG_PROBE_AWAIT(
            2, llg_arm_mailbox_put_value(
                   self, mailbox,
                   llg_mailbox_value_packed(payload, 129, 0, 0)));
    } else {
        sv4_t* selectors = llg_value_scope_values(llg_value_scope_begin(2));
        sv4_replace(&selectors[0], sv4_zero(32, 0));
        sv4_replace(&selectors[1], sv4_from_u64(1, 32, 0));
        int kind = mode >= 7 ? 3 : 0;
        if (mode % 2) llg_dyn_unstream_assign(&dynamic_array, payload, 1, 0, kind, selectors[0], selectors[1]);
        else llg_queue_unstream_assign(&queue, payload, 1, 0, kind, selectors[0], selectors[1]);
    }
    LLG_CO_EXIT_CHECK(ch);
    CHECK(mode == 4);
    LLG_PROBE_DONE();
}
static void native_callbacks(void) {
    for (mode = 0; mode < 9; ++mode) {
        for (unsigned round = 0; round < 8; ++round) {
            llg_rt_init(); evaluations = 0;
            sv4_replace(&watched, sv4_zero(129, 0));
            sv4_replace(&payload, sv4_from_u64(7, 129, 0));
            sv4_replace(&zero, sv4_zero(32, 0));
            destination = &watched;
            mailbox = llg_mailbox_new(zero, LLG_MAILBOX_PACKED, 129, 0, 0, 0);
            llg_dyn_init(&dynamic_array, 129, 0, 0);
            llg_queue_init(&queue, 129, 0, 0, UINT64_MAX);
            dynamic_array.contents_dependency = &watched;
            dynamic_array.notify = llg_dependency_notify;
            queue.contents_dependency = &watched;
            queue.notify = llg_dependency_notify;
            if (mode < 2)
                CHECK(llg_mailbox_try_put_value(
                    mailbox,
                    llg_mailbox_value_packed(payload, 129, 0, 0)));
            if (mode >= 2 && mode <= 4) llg_spawn(&receiver_desc, "mailbox receiver");
            llg_spawn(&observer_desc, "publication observer");
            llg_spawn(&publisher_desc, "mailbox/stream publisher");
            llg_rt_run();
            CHECK(evaluations == 2);
            llg_rt_cleanup();
            llg_process_assign(&receiver_handle, NULL);
            llg_dyn_destroy(&dynamic_array); llg_queue_destroy(&queue);
            sv4_destroy(&watched); sv4_destroy(&payload); sv4_destroy(&zero);
            if (value_test_live()) fprintf(stderr, "mode=%u round=%u\n", mode, round);
            CHECK(value_test_live() == 0 && value_test_bytes() == 0);
            CHECK(all_value_scopes == NULL && root_value_scopes == NULL);
        }
    }
}
int main(int argc, char** argv) {
    root_reference_scopes();
    if (argc < 2 || strcmp(argv[1], "--scopes-only")) native_callbacks();
    puts("native reference/mailbox/stream boundaries: OK");
    return 0;
}
