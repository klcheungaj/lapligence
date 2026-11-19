#include "llg_rt.c"
#include "probe.h"

/* The hot parts of concurrent assertions (scheduler/state.c,
 * llg_assertion_hot_t): one line-aligned array in registration order whose
 * entries and records point at each other through every growth of the
 * array, the per-publication walks that read them, and the list of the
 * assertions with an asynchronous accept_on/reject_on (g.async_aborts). */

#define CLOCKS 3u
#define ASSERTIONS 37u

static unsigned abort_calls;

static int predicate_false(void* data) {
    (void)data;
    return 0;
}

static int abort_holds(void* data) {
    (void)data;
    abort_calls++;
    return 1;
}

static sv4_t* clock_of(sv4_t* clocks, unsigned index) { return &clocks[index % CLOCKS]; }
static int edge_of(unsigned index) { return index % 2u ? LLG_EV_NEGEDGE : LLG_EV_POSEDGE; }
static int kind_of(unsigned index) {
    return index % 3u == 2u ? LLG_ASSERTION_COVER : LLG_ASSERTION_ASSERT;
}
static int disabled_by(unsigned index) { return index % 5u == 4u; }
/* Every fourth assertion has an accept_on/reject_on control; every other
 * one of those is synchronous. */
static int has_abort(unsigned index) { return index % 4u == 1u; }
static int abort_sync_of(unsigned index) { return index % 8u == 5u; }
/* Every seventh assertion is a sequence assertion (indices 3, 10, 17, ...),
 * whose one step is clocked by a posedge of clock 2: none of them has that
 * leading clock edge, so each takes it as an extra clock. */
static int sequence_of(unsigned index) { return index % 7u == 3u; }
#define EXTRA_CLOCK 2u

static sv4_t probe_clocks[CLOCKS];
static const llg_sequence_transition_t extra_step = {
    .from = 0, .to = 1, .min_delay = 1, .max_delay = 1, .atom = LLG_SEQUENCE_EPSILON,
    .clock = &probe_clocks[EXTRA_CLOCK], .edge = LLG_EV_POSEDGE};
static const llg_sequence_graph_t extra_clocked = {
    .states = 2, .start = 0, .accept = 1, .transition_count = 1, .transitions = &extra_step};

static void check_layout(size_t count) {
    CHECK(g.assertion_count == count);
    CHECK(g.assertion_hot_capacity >= count);
    CHECK((uintptr_t)g.assertion_hot % LLG_ASSERTION_HOT_ALIGN == 0);
    size_t index = 0;
    for (llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next, index++) {
        CHECK(index < count);
        CHECK(assertion->hot == &g.assertion_hot[index]);
        CHECK(assertion->hot->record == assertion);
    }
    CHECK(index == count);
    CHECK(count == 0 || g.assertion_tail->hot == &g.assertion_hot[count - 1u]);
    /* The asynchronous-abort list holds exactly the assertions with an
     * asynchronous control, in registration order. */
    const llg_concurrent_assertion_t* next_async = g.async_aborts;
    for (const llg_concurrent_assertion_t* assertion = g.assertions; assertion;
         assertion = assertion->next) {
        if (!assertion->hot->abort_condition || assertion->hot->abort_sync) continue;
        CHECK(next_async == assertion);
        if (!assertion->async_abort_next) CHECK(g.async_aborts_tail == assertion);
        next_async = assertion->async_abort_next;
    }
    CHECK(next_async == NULL);
    CHECK((g.async_aborts == NULL) == (g.async_aborts_tail == NULL));
}

static void check_fields(sv4_t* clocks, sv4_t* disable) {
    for (unsigned index = 0; index < ASSERTIONS; index++) {
        const llg_assertion_hot_t* hot = &g.assertion_hot[index];
        CHECK(hot->clock == clock_of(clocks, index));
        CHECK(hot->edge == edge_of(index));
        CHECK(hot->kind == kind_of(index));
        CHECK(hot->disable == (disabled_by(index) ? disable : NULL));
        CHECK(hot->abort_condition == (has_abort(index) ? abort_holds : NULL));
        CHECK(hot->abort_sync == (has_abort(index) && abort_sync_of(index)));
        CHECK(hot->enabled == 1 && hot->expect_active == 0);
        CHECK(hot->clock_gate == NULL);
        CHECK(hot->extra_clock_count == (sequence_of(index) ? 1u : 0u));
        CHECK(!sequence_of(index) || hot->extra_clocks[0].signal == &clocks[EXTRA_CLOCK]);
        CHECK(hot->record->identity == 100u + index);
        CHECK(hot->record->consequent_sequence == (sequence_of(index) ? &extra_clocked : NULL));
    }
}

/* A posedge of clock `clock` marks exactly the live assertions with that
 * leading or extra clock edge whose disable iff does not hold: a pending edge,
 * or a clock event of a sequence assertion. */
static void check_clock_walk(sv4_t* clocks, sv4_t* disable, unsigned off, unsigned clock) {
    sv4_t zero = sv4_zero(1, 0);
    sv4_t one = sv4_from_u64(1, 1, 0);
    for (int disabled = 0; disabled < 2; disabled++) {
        sv4_replace(disable, sv4_from_u64((uint64_t)disabled, 1, 0));
        for (llg_concurrent_assertion_t* a = g.assertions; a; a = a->next) {
            a->edge_pending = 0;
            free_assertion_clock_events(a);
        }
        g.assertion_edges_pending = 0;
        assertion_clock_signal_changed(&clocks[clock], zero, one);
        unsigned marked = 0;
        for (unsigned index = 0; index < ASSERTIONS; index++) {
            int leading = index % CLOCKS == clock && edge_of(index) == LLG_EV_POSEDGE;
            int extra = sequence_of(index) && clock == EXTRA_CLOCK;
            int expected = (leading || extra) && index != off && !(disabled && disabled_by(index));
            const llg_concurrent_assertion_t* record = g.assertion_hot[index].record;
            CHECK(record->edge_pending == (expected && !sequence_of(index)));
            CHECK((record->clock_events != NULL) == (expected && sequence_of(index)));
            marked += (unsigned)expected;
        }
        CHECK(marked != 0 && g.assertion_edges_pending == 1);
    }
    sv4_replace(disable, sv4_zero(1, 0));
    sv4_destroy(&zero);
    sv4_destroy(&one);
}

/* A publication evaluates the control of every enabled assertion with an
 * asynchronous accept_on/reject_on, and of no other. */
static void check_abort_walk(unsigned off) {
    unsigned expected = 0;
    for (unsigned index = 0; index < ASSERTIONS; index++)
        expected += (unsigned)(has_abort(index) && !abort_sync_of(index) && index != off);
    abort_calls = 0;
    assertion_abort_condition_changed();
    CHECK(abort_calls == expected && expected != 0);
}

static void exercise(void) {
    llg_rt_init();
    sv4_t* clocks = probe_clocks;
    for (unsigned index = 0; index < CLOCKS; index++) clocks[index] = sv4_zero(1, 0);
    sv4_t disable = sv4_zero(1, 0);
    check_layout(0);
    for (unsigned index = 0; index < ASSERTIONS; index++) {
        sv4_t* condition = disabled_by(index) ? &disable : NULL;
        llg_concurrent_assertion_predicate_fn control = has_abort(index) ? abort_holds : NULL;
        int sync = has_abort(index) && abort_sync_of(index);
        int reject = has_abort(index) && index % 3u == 0u;
        int ok;
        if (sequence_of(index))
            ok = llg_assertion_register_sequence_control(
                clock_of(clocks, index), edge_of(index), condition, NULL, &extra_clocked, control,
                NULL, NULL, NULL, kind_of(index), 1, 0, sync, 100u + index, "a", "l", "s");
        else
            ok = llg_assertion_register_control(
                clock_of(clocks, index), edge_of(index), condition, NULL, predicate_false,
                control, NULL, NULL, NULL, kind_of(index), 1, reject, sync, 100u + index, "a",
                "l", "s");
        CHECK(ok == 1);
        /* The array grows at 8, 16 and 32 entries and moves each time. */
        check_layout(index + 1u);
        /* The first asynchronous control is the second assertion's. */
        CHECK((g.async_aborts != NULL) == (index >= 1u));
    }
    CHECK(g.assertion_hot_capacity == 64);
    check_fields(clocks, &disable);
    for (unsigned clock = 0; clock < CLOCKS; clock++)
        check_clock_walk(clocks, &disable, ASSERTIONS, clock);
    check_abort_walk(ASSERTIONS);
    /* A disabled assertion without attempts takes no part in either walk. */
    const unsigned abort_off = 9, clock_off = 12;
    CHECK(has_abort(abort_off) && !abort_sync_of(abort_off));
    g.assertion_hot[abort_off].enabled = 0;
    check_abort_walk(abort_off);
    g.assertion_hot[abort_off].enabled = 1;
    CHECK(clock_off % CLOCKS == 0 && edge_of(clock_off) == LLG_EV_POSEDGE);
    g.assertion_hot[clock_off].enabled = 0;
    check_clock_walk(clocks, &disable, clock_off, 0);
    llg_rt_cleanup();
    CHECK(g.assertions == NULL && g.assertion_tail == NULL);
    CHECK(g.assertion_hot == NULL && g.assertion_hot_block == NULL);
    CHECK(g.assertion_count == 0 && g.assertion_hot_capacity == 0);
    CHECK(g.async_aborts == NULL && g.async_aborts_tail == NULL);
    for (unsigned index = 0; index < CLOCKS; index++) sv4_destroy(&clocks[index]);
    sv4_destroy(&disable);
}

int main(void) {
    /* A second run after cleanup starts from an empty array again. */
    for (int run = 0; run < 2; run++) exercise();
    CHECK(value_test_live() == 0);
    puts("assertion hot parts: OK");
    return 0;
}
