#include "llg_rt.c"
#include "probe.h"

/* The hot parts of concurrent assertions (scheduler/state.c,
 * llg_assertion_hot_t): one line-aligned array in registration order whose
 * entries and records point at each other through every growth of the
 * array, and the per-publication walks that read them. */

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
        CHECK(hot->clock_gate == NULL && hot->extra_clock_count == 0);
        CHECK(hot->record->identity == 100u + index);
    }
}

/* A posedge of clock 0 marks exactly the live assertions on that edge whose
 * disable iff does not hold. */
static void check_clock_walk(sv4_t* clocks, sv4_t* disable, unsigned off) {
    sv4_t zero = sv4_zero(1, 0);
    sv4_t one = sv4_from_u64(1, 1, 0);
    for (int disabled = 0; disabled < 2; disabled++) {
        sv4_replace(disable, sv4_from_u64((uint64_t)disabled, 1, 0));
        for (llg_concurrent_assertion_t* a = g.assertions; a; a = a->next) a->edge_pending = 0;
        g.assertion_edges_pending = 0;
        assertion_clock_signal_changed(&clocks[0], zero, one);
        unsigned marked = 0;
        for (unsigned index = 0; index < ASSERTIONS; index++) {
            int expected = index % CLOCKS == 0 && edge_of(index) == LLG_EV_POSEDGE &&
                           index != off && !(disabled && disabled_by(index));
            CHECK(g.assertion_hot[index].record->edge_pending == expected);
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
    sv4_t clocks[CLOCKS];
    for (unsigned index = 0; index < CLOCKS; index++) clocks[index] = sv4_zero(1, 0);
    sv4_t disable = sv4_zero(1, 0);
    check_layout(0);
    for (unsigned index = 0; index < ASSERTIONS; index++) {
        sv4_t* condition = disabled_by(index) ? &disable : NULL;
        int ok = has_abort(index)
                     ? llg_assertion_register_control(
                           clock_of(clocks, index), edge_of(index), condition, NULL,
                           predicate_false, abort_holds, NULL, NULL, NULL, kind_of(index), 1,
                           index % 3u == 0u, abort_sync_of(index), 100u + index, "a", "l", "s")
                     : llg_assertion_register(clock_of(clocks, index), edge_of(index), condition,
                                              NULL, predicate_false, NULL, NULL, NULL,
                                              kind_of(index), 1, 100u + index, "a", "l", "s");
        CHECK(ok == 1);
        /* The array grows at 8, 16 and 32 entries and moves each time. */
        check_layout(index + 1u);
    }
    CHECK(g.assertion_hot_capacity == 64);
    check_fields(clocks, &disable);
    check_clock_walk(clocks, &disable, ASSERTIONS);
    check_abort_walk(ASSERTIONS);
    /* A disabled assertion without attempts takes no part in either walk. */
    const unsigned abort_off = 9, clock_off = 12;
    CHECK(has_abort(abort_off) && !abort_sync_of(abort_off));
    g.assertion_hot[abort_off].enabled = 0;
    check_abort_walk(abort_off);
    g.assertion_hot[abort_off].enabled = 1;
    CHECK(clock_off % CLOCKS == 0 && edge_of(clock_off) == LLG_EV_POSEDGE);
    g.assertion_hot[clock_off].enabled = 0;
    check_clock_walk(clocks, &disable, clock_off);
    llg_rt_cleanup();
    CHECK(g.assertions == NULL && g.assertion_tail == NULL);
    CHECK(g.assertion_hot == NULL && g.assertion_hot_block == NULL);
    CHECK(g.assertion_count == 0 && g.assertion_hot_capacity == 0);
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
