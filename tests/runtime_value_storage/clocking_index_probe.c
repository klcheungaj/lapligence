#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static void check_current(sv4_t* signal, int kind, int expected) {
    llg_wait_src_t source = {signal, kind, NULL};
    CHECK(clocking_event_current(&source, 1) == expected);
}

// Records an edge of a registered descriptor.
static void record_registered(sv4_t* signal, sv4_t old, sv4_t value) {
    llg_rt_clocking_history_signal(signal);
    clocking_record_edge(signal, old, value);
}

static void exercise_index(void) {
    const size_t count = 4096;
    sv4_t* signals = calloc(count, sizeof(*signals));
    CHECK(signals != NULL);
    sv4_t zero = sv4_zero(1, 0);
    sv4_t one = sv4_from_u64(1, 1, 0);
    sv4_t unknown = sv4_x(1, 0);
    llg_rt_init();
    g.now = 7;
    /* Publication of storage nobody registered records nothing. */
    for (size_t i = 0; i < count; ++i) clocking_record_edge(&signals[i], zero, one);
    CHECK(g.clocking_count == 0 && g.clocking_index == NULL);
    llg_rt_clocking_history_signal(NULL);
    CHECK(g.clocking_count == 0);
    for (size_t i = 0; i < count; ++i) {
        record_registered(&signals[i], zero, one);
        CHECK(find_clocking_edge(&signals[i])->signal == &signals[i]);
    }
    CHECK(g.clocking_count == count);
    /* A second registration keeps the entry and its occurrence. */
    llg_rt_clocking_history_signal(&signals[1]);
    CHECK(g.clocking_count == count);
    check_current(&signals[1], LLG_EV_POSEDGE, 1);
    for (size_t i = 0; i < count; i += 2) {
        llg_clocking_forget_signal(&signals[i]);
        CHECK(find_clocking_edge(&signals[i]) == NULL);
        llg_clocking_forget_signal(&signals[i]);
    }
    CHECK(g.clocking_count == count / 2);
    for (size_t i = 1; i < count; i += 2) check_current(&signals[i], LLG_EV_POSEDGE, 1);
    for (size_t i = 0; i < count; i += 2) {
        /* Forgotten storage is unregistered again. */
        clocking_record_edge(&signals[i], one, zero);
        CHECK(find_clocking_edge(&signals[i]) == NULL);
        record_registered(&signals[i], one, zero);
        check_current(&signals[i], LLG_EV_POSEDGE, 0);
        check_current(&signals[i], LLG_EV_NEGEDGE, 1);
    }
    CHECK(g.clocking_count == count);
    llg_clocking_edge_t* persistent = clocking_edge_get(&signals[1]);
    persistent->keep_ticks = 1;
    clocking_record_edge(&signals[1], one, zero);
    clocking_record_edge(&signals[1], zero, unknown);
    clocking_record_edge(&signals[1], unknown, one);
    CHECK(g.clocking_count == count);
    CHECK(assertion_clock_tick(&signals[1], LLG_EV_POSEDGE) == 2);
    CHECK(assertion_clock_tick(&signals[1], LLG_EV_NEGEDGE) == 1);
    CHECK(assertion_clock_tick(&signals[3], LLG_EV_POSEDGE) == 0);
    check_current(&signals[1], LLG_EV_ANY, 1);
    check_current(&signals[1], LLG_EV_POSEDGE, 1);
    check_current(&signals[1], LLG_EV_NEGEDGE, 1);
    for (size_t i = 2; i < count; ++i) llg_clocking_forget_signal(&signals[i]);
    CHECK(g.clocking_count == 2);
    /* Time advance compacts the tombstones; registrations survive it and
     * their occurrences expire with the time step. */
    clocking_advance_time();
    ++g.now;
    CHECK(g.clocking_count == 2);
    CHECK(g.clocking_used == 2);
    CHECK(g.clocking_capacity == CLOCKING_INDEX_INITIAL_CAPACITY);
    CHECK(find_clocking_edge(&signals[2]) == NULL);
    CHECK(find_clocking_edge(&signals[0]) != NULL);
    check_current(&signals[0], LLG_EV_ANY, 0);
    check_current(&signals[1], LLG_EV_ANY, 0);
    CHECK(assertion_clock_tick(&signals[1], LLG_EV_POSEDGE) == 2);
    clocking_record_edge(&signals[1], zero, one);
    CHECK(assertion_clock_tick(&signals[1], LLG_EV_POSEDGE) == 3);
    check_current(&signals[1], LLG_EV_POSEDGE, 1);
    check_current(&signals[1], LLG_EV_NEGEDGE, 0);
    clocking_record_edge(&signals[0], one, zero);
    check_current(&signals[0], LLG_EV_NEGEDGE, 1);
    persistent->posedge_count = UINT64_MAX - 1;
    clocking_record_edge(&signals[1], zero, one);
    clocking_record_edge(&signals[1], zero, one);
    CHECK(assertion_clock_tick(&signals[1], LLG_EV_POSEDGE) == UINT64_MAX);

    llg_value_scope_t* scope = llg_value_scope_begin(1);
    sv4_t* local = scope->values;
    record_registered(local, zero, one);
    sv4_copy(local, &one);
    llg_nba_after(local, zero, 0);
    llg_value_scope_end(scope);
    CHECK(scope->references == 1);
    check_current(local, LLG_EV_POSEDGE, 1);
    size_t before_release = g.clocking_count;
    commit_nbas(LLG_REGION_NBA);
    CHECK(g.clocking_count == before_release - 1);
    for (size_t i = 0; i < count; ++i) {
        scope = llg_value_scope_begin(1);
        local = scope->values;
        /* This also exercises actual allocator address reuse after final free;
         * every new owner must start with no history, including recycled cells. */
        CHECK(find_clocking_edge(local) == NULL);
        record_registered(local, zero, one);
        llg_value_scope_end(scope);
        CHECK(g.clocking_count == 2);
    }
    /* Deterministic descriptor-address reuse with an independent new owner. */
    sv4_t* external = malloc(sizeof(*external));
    CHECK(external != NULL);
    *external = sv4_zero(1, 0);
    record_registered(external, zero, one);
    llg_clocking_forget_signal(external);
    sv4_destroy(external);
    *external = sv4_zero(1, 0);
    check_current(external, LLG_EV_POSEDGE, 0);
    record_registered(external, one, zero);
    check_current(external, LLG_EV_NEGEDGE, 1);
    llg_clocking_forget_signal(external);
    sv4_destroy(external);
    free(external);
    llg_clocking_forget_signal(NULL);
    llg_rt_cleanup();
    CHECK(g.clocking_index == NULL && g.clocking_edges == NULL);
    CHECK(g.clocking_count == 0 && g.clocking_used == 0);
    free(signals);
    sv4_destroy(&zero);
    sv4_destroy(&one);
    sv4_destroy(&unknown);
    CHECK(value_test_live() == 0);
}

static sv4_t scheduler_clock;
LLG_PROBE_SIMPLE_PROCESS(time_advance, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    check_current(&scheduler_clock, LLG_EV_POSEDGE, 1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    CHECK(g.now == 1);
    /* The registration outlives the time step; its occurrence does not. */
    CHECK(find_clocking_edge(&scheduler_clock) != NULL);
    check_current(&scheduler_clock, LLG_EV_POSEDGE, 0);
    llg_rt_finish();
    LLG_PROBE_EXIT();
}

int main(void) {
    for (unsigned cycle = 0; cycle < 4; ++cycle) exercise_index();
    llg_rt_init();
    sv4_t zero = sv4_zero(1, 0);
    sv4_t one = sv4_from_u64(1, 1, 0);
    record_registered(&scheduler_clock, zero, one);
    llg_spawn(&time_advance_desc, "time advance");
    llg_rt_run();
    llg_rt_cleanup();
    sv4_destroy(&zero);
    sv4_destroy(&one);
    CHECK(value_test_live() == 0);
    puts("clocking history index: OK");
    return 0;
}
