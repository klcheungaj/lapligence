/* Runtime-only regressions for the post-batch-5 source review. */
#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static unsigned destroyed;
static void destroy_real(void* payload) {
    (void)payload;
    ++destroyed;
}

static void root_storage(void) {
    enum { COUNT = 257 };
    for (unsigned round = 0; round < 8; ++round) {
        llg_value_scope_t* owners[COUNT];
        llg_value_scope_t* retained[COUNT];
        double* values[COUNT];
        double global = 0.0;
        llg_rt_init(); destroyed = 0;
        for (size_t i = 0; i < COUNT; ++i) {
            owners[i] = llg_value_scope_begin_object(sizeof(double), destroy_real);
            values[i] = llg_value_scope_object(owners[i]);
            *values[i] = (double)i;
            CHECK(value_scope_index_find(values[i]) == owners[i]);
            retained[i] = value_scope_retain_target(values[i]);
            CHECK(retained[i] == owners[i]);
        }
        CHECK(value_scope_index_find(&global) == NULL);
        CHECK(value_target_pin(&global) == NULL);
        for (size_t i = 0; i < COUNT; ++i) llg_value_scope_end(owners[i]);
        CHECK(destroyed == 0);
        /* Odd/even removals exercise tombstones alongside retained native keys. */
        for (size_t i = 0; i < COUNT; i += 2) value_scope_release(retained[i]);
        for (size_t i = 1; i < COUNT; i += 2) {
            CHECK(value_scope_index_find(values[i]) == retained[i]);
            CHECK(*values[i] == (double)i);
            value_scope_release(retained[i]);
        }
        CHECK(destroyed == COUNT);
        llg_value_scope_t* empty = llg_value_scope_begin_object(0, NULL);
        CHECK(llg_value_scope_object(empty) == NULL);
        llg_value_scope_end(empty);
        CHECK(value_scope_count == 0 && value_scope_index == NULL);
        CHECK(root_value_scopes == NULL && all_value_scopes == NULL);

        llg_value_scope_t* packed = llg_value_scope_begin(2);
        sv4_t* data = llg_value_scope_values(packed);
        sv4_replace(&data[0], sv4_zero(65, 0));
        sv4_replace(&data[1], sv4_from_u64(1, 1, 0));
        llg_ref_t ref = {0};
        ref.kind = LLG_REF_WHOLE; ref.width = 65; ref.base = &data[0];
        llg_ref_write_bit(&ref, UINT64_C(64), data[1]);
        expect_number(sv4_bit_select(data[0], UINT64_C(64)), 1);
        llg_ref_write_bit(&ref, UINT64_MAX, data[1]);
        llg_ref_write_bit(&ref, UINT64_C(65), data[1]);
        CHECK(sv4_to_u64(data[0]) == 0);
        expect_number(sv4_bit_select(data[0], UINT64_C(64)), 1);
        llg_value_scope_end(packed);
        llg_rt_cleanup();
        CHECK(value_scope_count == 0 && all_value_scopes == NULL);
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
}

static llg_mailbox_t* mailbox;
static llg_process_handle_t* receiver_handle;
static double* destination;
static double observed;
static double sent;
static unsigned evaluations;
static unsigned resumed;
static unsigned mode;

static void real_evaluator(double* out, void* context) {
    (void)context;
    ++evaluations;
    *out = *destination;
}

typedef struct {
    llg_co_frame_t co;
    double* local;
} review_receiver_frame_t;
LLG_CO_ROOT_FRAME_OK(review_receiver_frame_t);

static double* receiver_setup(llg_proc_t* self) {
    llg_value_scope_t* owner =
        llg_value_scope_begin_object(sizeof(double), destroy_real);
    double* local = (double*)llg_value_scope_object(owner);
    *local = 0.0;
    destination = local;
    llg_process_assign(&receiver_handle, llg_process_self(self));
    return local;
}

LLG_PROBE_PROCESS(receiver, review_receiver_frame_t, 1) {
    LLG_PROBE_BEGIN(review_receiver_frame_t, 1);
    /* This is the new emitted storage pattern, not a call to the Rust emitter. */
    F->local = receiver_setup(self);
    LLG_PROBE_AWAIT(
        1, llg_arm_mailbox_get_value(
               self, mailbox, llg_mailbox_target_real(F->local, mode == 1), 0));
    /* A woken get writes its destination when it resumes. */
    llg_mailbox_delivery_take(self, llg_mailbox_target_real(F->local, mode == 1));
    CHECK(mode < 2);
    observed = *F->local;
    ++resumed;
    LLG_PROBE_DONE();
}

static llg_co_arm_t arm_observer(llg_proc_t* self) {
    llg_wait_dependency_t dependency = {0};
    dependency.real = destination;
    llg_expr_event_spec_t event = {0};
    event.real = 1; event.real_eval = real_evaluator; event.kind = LLG_EV_ANY;
    event.dependencies = &dependency; event.n_dependencies = 1;
    return llg_arm_expressions(self, &event, 1);
}

LLG_PROBE_SIMPLE_PROCESS(observer, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, arm_observer(self));
    CHECK(mode == 2);
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(writer, 2) {
    LLG_PROBE_SIMPLE_BEGIN(2);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    LLG_PROBE_AWAIT(
        2, llg_arm_mailbox_put_value(
               self, mailbox, llg_mailbox_value_real(sent, mode == 1)));
    if (mode >= 2) {
        /* The receiver is woken with a pending delivery but has not written
         * its live storage yet. Killing it hands the message back (mode 2);
         * finishing drops the pending delivery at teardown (mode 3). */
        CHECK(evaluations == 1 && *destination == 0.0 && destroyed == 0);
        if (mode == 2) {
            llg_process_kill(self, receiver_handle);
            CHECK(llg_mailbox_num(mailbox) == 1);
        }
        llg_rt_finish();
        LLG_CO_EXIT_CHECK(ch);
        CHECK(0);
    }
    LLG_PROBE_DONE();
}

static void real_coroutines(void) {
    for (mode = 0; mode < 4; ++mode) {
        for (unsigned round = 0; round < 8; ++round) {
            llg_rt_init(); destroyed = evaluations = resumed = 0;
            observed = -1.0;
            sent = mode == 1 ? 7.25000001 : 7.25;
            sv4_t zero = sv4_zero(32, 0);
            mailbox = llg_mailbox_new(zero, LLG_MAILBOX_UNTYPED, 0, 0, 0, 0);
            sv4_destroy(&zero);
            llg_spawn(&receiver_desc, "review real receiver");
            if (mode >= 2) llg_spawn(&observer_desc, "review real observer");
            llg_spawn(&writer_desc, "review real writer");
            llg_rt_run();
            if (mode < 2) {
                CHECK(resumed == 1);
                CHECK(observed == (mode == 1 ? (double)(float)sent : sent));
            } else {
                CHECK(resumed == 0 && evaluations == 1);
            }
            llg_rt_cleanup();
            llg_process_assign(&receiver_handle, NULL);
            CHECK(destroyed == 1);
            CHECK(root_value_scopes == NULL && all_value_scopes == NULL);
            CHECK(value_scope_count == 0 && value_scope_index == NULL);
            CHECK(value_test_live() == 0 && value_test_bytes() == 0);
        }
    }
}

int main(int argc, char** argv) {
    if (argc == 2 && strcmp(argv[1], "--root-only") == 0) {
        root_storage();
    } else {
        CHECK(argc == 1);
        real_coroutines();
    }
    return 0;
}
