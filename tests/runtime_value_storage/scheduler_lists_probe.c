/* Production bookkeeping with exact order oracles, arbitrary removal, nested
 * publication cursors, duplicate dependencies, growth and repeated teardown. */
#include "llg_rt.c"
#include "probe.h"

static llg_proc_t* test_proc(void) {
    llg_proc_t* proc = calloc(1, sizeof(*proc));
    CHECK(proc != NULL);
    register_proc(proc);
    proc->handle = process_handle_new(proc);
    proc->region = LLG_REGION_ACTIVE;
    return proc;
}

static void arm_signals(llg_proc_t* proc, sv4_t** signals, int count) {
    g.current = proc;
    CHECK(llg_arm_any(proc, signals, count) == LLG_CO_ARM_SUSPEND);
    g.current = NULL;
}

static void registry_and_handles(void) {
    enum { COUNT = 20000 };
    llg_proc_t** procs = calloc(COUNT, sizeof(*procs));
    CHECK(procs != NULL);
    llg_rt_init();
    for (int i = 0; i < COUNT; i++) {
        procs[i] = test_proc();
        CHECK(procs[i]->registry_slot == i);
    }
    // Remove ascending interior holes, then refill in lowest-slot order.
    for (int i = 1; i < COUNT - 1; i += 2) unregister_proc(procs[i]);
    for (int i = 1; i < COUNT - 1; i += 2) {
        register_proc(procs[i]);
        CHECK(procs[i]->registry_slot == i);
    }
    llg_process_handle_t* middle = procs[COUNT / 2]->handle;
    process_handle_unlink(middle);
    process_handle_unlink(middle);
    CHECK(!middle->linked && !middle->prev_link);
    // Releasing an already unlinked identity remains harmless.
    process_handle_shutdown(procs[COUNT / 2]);
    process_handle_shutdown(procs[0]); // oldest / tail
    process_handle_shutdown(procs[COUNT - 1]); // newest / head
    for (int i = COUNT; i > 0; i--) {
        CHECK(g.n_procs == i);
        unregister_proc(procs[i - 1]);
    }
    CHECK(g.n_procs == 0 && proc_first_free() == 0);
    for (int i = 0; i < COUNT; i++) {
        register_proc(procs[i]);
        CHECK(procs[i]->registry_slot == i);
    }
    llg_process_handle_t* retained = procs[7]->handle;
    llg_process_retain(retained);
    llg_rt_cleanup();
    CHECK(!retained->linked && !retained->prev_link && retained->proc == NULL);
    llg_process_release(retained);
    free(procs);
    CHECK(value_test_live() == 0);
}

static void waiter_order_and_cursors(void) {
    sv4_t signal = sv4_zero(1, 0);
    sv4_t other = sv4_zero(1, 0);
    sv4_t one = sv4_from_u64(1, 1, 0);
    llg_rt_init();
    llg_proc_t* procs[5];
    sv4_t* duplicate[] = {&signal, &other, &signal, &other};
    for (int i = 0; i < 5; i++) {
        procs[i] = test_proc();
        arm_signals(procs[i], duplicate, 4);
        CHECK(procs[i]->wait.n_subscriptions == 2);
    }
    CHECK(g.n_wait_sources == 2);
    llg_wait_source_t* source = wait_source_find(&signal);
    llg_wait_subscription_t cursor = {0};
    cursor.source = source;
    wait_subscription_insert(&cursor, &source->head);
    CHECK(wait_source_next(&cursor) == &procs[4]->wait);
    // Cancellation of the next waiter repairs the cursor's forward link.
    llg_kill_proc(procs[3], 0);
    CHECK(wait_source_next(&cursor) == &procs[2]->wait);
    // Nested publication removes current, preceding and following entries.
    sig_write(&signal, one);
    CHECK(wait_source_next(&cursor) == NULL);
    CHECK(g.n_wait_sources == 1); // the outer cursor alone retains this row
    wait_subscription_unlink(&cursor);
    CHECK(g.n_wait_sources == 0 && g.waiters == NULL && g.wait_count == 0);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == procs[4]);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == procs[2]);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == procs[1]);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == procs[0]);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == NULL);
    reap_retired_procs();
    // Explicit head/middle/tail removal leaves survivor order unchanged.
    for (int i = 0; i < 5; i++) {
        procs[i] = test_proc();
        arm_signals(procs[i], duplicate, 4);
    }
    wake_proc(procs[4]);
    wake_proc(procs[2]);
    wake_proc(procs[0]);
    source = wait_source_find(&other);
    CHECK(source->head->wait == &procs[3]->wait);
    CHECK(source->head->next->wait == &procs[1]->wait);
    CHECK(source->head->next->next == NULL);
    llg_rt_cleanup(); // surviving waits and all memberships are unlinked
    CHECK(!g.wait_sources && g.n_wait_sources == 0);
    sv4_destroy(&signal);
    sv4_destroy(&other);
    sv4_destroy(&one);
    CHECK(value_test_live() == 0);
}

static void many_dependencies(void) {
    enum { COUNT = 20000 };
    sv4_t* signals = calloc(COUNT, sizeof(*signals));
    sv4_t** keys = calloc(COUNT + 2, sizeof(*keys));
    CHECK(signals != NULL && keys != NULL);
    llg_rt_init();
    for (int i = 0; i < COUNT; i++) {
        signals[i] = sv4_zero(1, 0);
        keys[i] = &signals[i];
    }
    keys[COUNT] = keys[0];
    keys[COUNT + 1] = keys[COUNT / 2];
    llg_proc_t* proc = test_proc();
    arm_signals(proc, keys, COUNT + 2);
    CHECK(proc->wait.n_subscriptions == COUNT && g.n_wait_sources == COUNT);
    sv4_t one = sv4_from_u64(1, 1, 0);
    sig_write(&signals[COUNT / 2], one);
    CHECK(g.n_wait_sources == 0 && g.wait_count == 0);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == proc);
    // Reuse the grown hash table with entirely fresh rows repeatedly.
    for (int i = 0; i < 100; i++) {
        arm_signals(proc, keys + i, 1);
        wake_proc(proc);
        CHECK(dequeue_region(LLG_REGION_ACTIVE) == proc);
        CHECK(g.n_wait_sources == 0);
    }
    llg_rt_cleanup();
    sv4_destroy_array(signals, COUNT);
    sv4_destroy(&one);
    free(keys);
    free(signals);
    CHECK(value_test_live() == 0);
}

static void many_waiters(void) {
    enum { COUNT = 20000 };
    llg_proc_t** procs = calloc(COUNT, sizeof(*procs));
    CHECK(procs != NULL);
    sv4_t signal = sv4_zero(1, 0);
    sv4_t* keys[] = {&signal};
    sv4_t one = sv4_from_u64(1, 1, 0);
    llg_rt_init();
    for (int i = 0; i < COUNT; i++) {
        procs[i] = test_proc();
        arm_signals(procs[i], keys, 1);
    }
    CHECK(g.n_wait_sources == 1 && g.wait_count == COUNT);
    // Cancel interior waiters before publication; survivors retain LIFO order.
    for (int i = 1; i < COUNT; i += 2) llg_kill_proc(procs[i], 0);
    sig_write(&signal, one);
    for (int i = COUNT - 2; i >= 0; i -= 2)
        CHECK(dequeue_region(LLG_REGION_ACTIVE) == procs[i]);
    CHECK(!g.waiters && !g.n_wait_sources && !g.wait_count);
    llg_rt_cleanup();
    free(procs);
    sv4_destroy(&signal);
    sv4_destroy(&one);
    CHECK(value_test_live() == 0);
}

static llg_proc_t* cancel_during_publish;
static sv4_t* publication_signal;

static void cancelling_evaluator(sv4_t* out, void* context) {
    (void)context;
    sv4_copy(out, publication_signal);
    if (cancel_during_publish) {
        llg_kill_proc(cancel_during_publish, 0);
        cancel_during_publish = NULL;
    }
}

static void removal_during_publish(void) {
    sv4_t signal = sv4_zero(1, 0);
    sv4_t one = sv4_from_u64(1, 1, 0);
    sv4_t* keys[] = {&signal};
    llg_rt_init();
    llg_proc_t* tail = test_proc();
    llg_proc_t* victim = test_proc();
    llg_proc_t* head = test_proc();
    arm_signals(tail, keys, 1);
    arm_signals(victim, keys, 1);
    llg_expr_event_spec_t spec = {0};
    spec.sig = &signal;
    spec.kind = LLG_EV_ANY;
    publication_signal = &signal;
    g.current = head;
    CHECK(llg_arm_expressions(head, &spec, 1) == LLG_CO_ARM_SUSPEND);
    g.current = NULL;
    head->wait.payload.expression.expressions[0].eval = cancelling_evaluator;
    // The callback uses the global signal below; no context ownership changes.
    cancel_during_publish = victim;
    sig_write(&signal, one);
    CHECK(!cancel_during_publish && !g.waiters && !g.n_wait_sources);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == head);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == tail);
    llg_rt_cleanup();
    sv4_destroy(&signal);
    sv4_destroy(&one);
    CHECK(value_test_live() == 0);
}

static void activation_order(void) {
    enum { COUNT = 20000 };
    llg_activation_t** entries = calloc(COUNT, sizeof(*entries));
    CHECK(entries != NULL);
    llg_rt_init();
    llg_proc_t* proc = test_proc();
    g.current = proc;
    for (int i = 0; i < COUNT; i++)
        entries[i] = llg_activation_enter((uint32_t)i, 0);
    CHECK(g.activations == entries[COUNT - 1]);
    llg_activation_exit(entries[COUNT / 2]);
    llg_activation_exit(entries[0]);
    llg_activation_exit(entries[COUNT - 1]);
    CHECK(g.activations == entries[COUNT - 2]);
    CHECK(proc->activation_top == entries[COUNT - 2]);
    CHECK(entries[COUNT / 2 + 1]->all_next == entries[COUNT / 2 - 1]);
    CHECK(entries[COUNT / 2 + 1]->proc_next == entries[COUNT / 2 - 1]);
    // Parents retained by descendants are released after unlink, not reused.
    g.current = NULL;
    llg_rt_cleanup();
    CHECK(g.activations == NULL);
    free(entries);
    CHECK(value_test_live() == 0);
}

static void process_await_order(void) {
    llg_rt_init();
    llg_proc_t* target = test_proc();
    llg_proc_t* waiters[3];
    for (int i = 0; i < 3; i++) {
        waiters[i] = test_proc();
        g.current = waiters[i];
        CHECK(llg_arm_process_await(waiters[i], target->handle) == LLG_CO_ARM_SUSPEND);
    }
    g.current = NULL;
    process_handle_terminal(target, LLG_PROCESS_FINISHED);
    CHECK(g.n_wait_sources == 0);
    for (int i = 2; i >= 0; i--)
        CHECK(dequeue_region(LLG_REGION_ACTIVE) == waiters[i]);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
}

int main(void) {
    for (int cycle = 0; cycle < 3; cycle++) {
        registry_and_handles();
        waiter_order_and_cursors();
        many_dependencies();
        many_waiters();
        removal_during_publish();
        activation_order();
        process_await_order();
    }
    printf("scheduler lists: OK (proc=%zu wait=%zu handle=%zu activation=%zu subscription=%zu source=%zu)\n",
           sizeof(llg_proc_t), sizeof(llg_wait_t), sizeof(llg_process_handle_t),
           sizeof(llg_activation_t), sizeof(llg_wait_subscription_t), sizeof(llg_wait_source_t));
    return 0;
}
