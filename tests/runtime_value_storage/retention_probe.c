/* Runtime state created per simulated step must be released while the
 * simulation runs: completed top-level processes (assertion actions, detached
 * spawns) and sampled-value histories stay bounded however long time advances.
 * Counts are checked after every step, so any per-step retention fails. */
#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

enum { RETENTION_STEPS = 2000, DOMAIN_TICKS = 3, DOMAIN_STEPS = 1000 };

static sv4_t target;
static uint64_t issued;
static unsigned children_done;
static unsigned parent_checks;

// Completes in its first turn with a same-slot NBA still queued, so the
// write must commit after the issuing record has been retired.
LLG_PROBE_SIMPLE_PROCESS(nba_child, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    sv4_t value = sv4_from_u64(++issued, 32, 0);
    llg_nba(&target, value);
    sv4_destroy(&value);
    children_done++;
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(spawn_driver, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    for (F->index = 0; F->index < RETENTION_STEPS; F->index++) {
        CHECK(llg_spawn_in_region(&nba_child_desc, "active child", LLG_REGION_ACTIVE));
        CHECK(llg_spawn_in_region(&nba_child_desc, "reactive child", LLG_REGION_REACTIVE));
        CHECK(llg_spawn_detached_with_frame(&nba_child_desc, "detached child", NULL));
        LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
        // Only this driver remains registered; every child record is gone.
        CHECK(children_done == 3u * ((unsigned)F->index + 1u));
        CHECK(llg_rt_process_count() == 1);
        CHECK(g.n_procs == 1 && !g.retired_procs);
        CHECK(sv4_to_u64(target) == issued);
    }
    LLG_PROBE_DONE();
}

static void completed_spawns_are_reclaimed(void) {
    llg_rt_init();
    target = sv4_zero(32, 0);
    issued = 0;
    children_done = 0;
    CHECK(llg_spawn(&spawn_driver_desc, "driver"));
    llg_rt_run();
    CHECK(children_done == 3u * RETENTION_STEPS);
    CHECK(g.n_procs == 0);
    llg_rt_cleanup();
    sv4_destroy(&target);
    CHECK(value_test_live() == 0);
}

LLG_PROBE_SIMPLE_PROCESS(late_child, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 2));
    children_done++;
    LLG_PROBE_DONE();
}

// A completed top-level parent still owns the live join_none group that its
// child reports to, so its record must survive until the group detaches.
LLG_PROBE_SIMPLE_PROCESS(join_none_parent, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    llg_fork_group_t* group = llg_fork_group_new(LLG_JOIN_NONE);
    CHECK(llg_fork(&late_child_desc, "late child", group));
    CHECK(llg_arm_join(self, group) == LLG_CO_ARM_READY);
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(parent_checker, 2) {
    LLG_PROBE_SIMPLE_BEGIN(2);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    CHECK(llg_rt_process_count() == 3 && children_done == 0);
    parent_checks++;
    LLG_PROBE_AWAIT(2, llg_arm_time(self, 2));
    CHECK(children_done == 1);
    CHECK(llg_rt_process_count() == 1 && !g.retired_procs && !g.zombie_groups);
    parent_checks++;
    LLG_PROBE_DONE();
}

static void completed_parent_waits_for_join_none_child(void) {
    llg_rt_init();
    children_done = 0;
    parent_checks = 0;
    CHECK(llg_spawn(&parent_checker_desc, "checker"));
    CHECK(llg_spawn(&join_none_parent_desc, "parent"));
    llg_rt_run();
    CHECK(parent_checks == 2);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
}

static sv4_t domain_clock;
static uint64_t domain_sample;

static sv4_t domain_value(void* data) {
    (void)data;
    return sv4_from_u64(domain_sample, 16, 0);
}

// The newest sample of step `step`; every seventh step has a second edge.
static uint64_t sample_at(uint64_t step) {
    return step * 10u + (step % 7u == 0 ? 1u : 0u);
}

static void drive_clock(uint64_t level) {
    sv4_t value = sv4_from_u64(level, 1, 0);
    llg_ba(&domain_clock, value);
    sv4_destroy(&value);
}

static void sampled_domain_history_is_bounded(void) {
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    domain_clock = sv4_zero(1, 0);
    domain_sample = 0;
    CHECK(llg_sampled_domain_register(7, &domain_clock, LLG_EV_POSEDGE, domain_value,
                                      NULL, NULL, DOMAIN_TICKS));
    llg_sampled_domain_t* domain = find_sampled_domain(7);
    CHECK(domain != NULL);
    for (uint64_t step = 1; step <= DOMAIN_STEPS; step++) {
        g.now = step;
        domain_sample = step * 10u;
        drive_clock(1);
        if (step % 7u == 0) {
            drive_clock(0);
            domain_sample = sample_at(step);
            drive_clock(1);
        }
        // $past skips the evaluating step and reads the newest sample of the
        // k-th earlier one, or the registration value before enough ticks.
        for (uint64_t ticks = 1; ticks <= DOMAIN_TICKS; ticks++)
            expect_number(llg_sampled_domain_past(7, ticks),
                          step > ticks ? sample_at(step - ticks) : 0);
        CHECK(llg_sampled_domain_status(7, 3) == 1); // $changed
        drive_clock(0);
        size_t entries = 0;
        for (llg_sampled_domain_history_t* history = domain->history; history;
             history = history->next)
            entries++;
        CHECK(domain->history_groups <= DOMAIN_TICKS + 1);
        CHECK(entries <= 2u * (DOMAIN_TICKS + 1));
    }
    llg_rt_cleanup();
    sv4_destroy(&domain_clock);
    CHECK(value_test_live() == 0);
}

static size_t history_entries(const sv4_t* signal) {
    size_t entries = 0;
    for (llg_sampled_history_t* history = find_sampled_value(signal)->history; history;
         history = history->next)
        entries++;
    return entries;
}

// Clocking input skews read the newest slot sample at or before `now - ticks`;
// the source keeps only the slots its deepest registered skew can select.
static void clocking_history_is_bounded(void) {
    enum { SKEW = 5, SLOTS = 1000 };
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    sv4_t skewed = sv4_zero(32, 0);
    sv4_t plain = sv4_zero(32, 0);
    sv4_t sample = sv4_zero(32, 0);
    llg_sampled_register(&skewed);
    llg_sampled_register_history(&skewed, 2);
    llg_sampled_register_history(&skewed, SKEW);
    llg_sampled_register_history(&skewed, 1); // never lowers the depth
    llg_sampled_register(&plain);
    for (uint64_t slot = 1; slot <= SLOTS; slot++) {
        // Activity only in even ticks, so odd skews fall between slots.
        g.now = 2 * slot;
        sample_preponed_values();
        sv4_t value = sv4_from_u64(3 * g.now, 32, 0);
        llg_ba(&skewed, value);
        llg_ba(&plain, value);
        sv4_destroy(&value);
        for (uint64_t ticks = 0; ticks <= SKEW; ticks++) {
            CHECK(llg_clocking_sample_history(&skewed, &sample, ticks));
            uint64_t selected = g.now < ticks ? 0 : g.now - ticks;
            CHECK(sv4_to_u64(sample) == 3 * (selected - selected % 2));
        }
        CHECK(history_entries(&skewed) <= SKEW + 2);
        CHECK(history_entries(&plain) == 1);
    }
    llg_rt_cleanup();
    sv4_destroy(&sample);
    sv4_destroy(&plain);
    sv4_destroy(&skewed);
    CHECK(value_test_live() == 0);
}

// Assertions register every signal value-only; a clocking skew registration
// of the same signal then adds history without leaving a duplicate snapshot.
static void value_registration_is_promoted(void) {
    llg_rt_init();
    sv4_t source = sv4_from_u64(5, 8, 0);
    llg_sampled_register_value(&source);
    CHECK(g.sampled_values && !g.sampled);
    llg_sampled_register(&source);
    CHECK(!g.sampled_values && g.sampled && g.sampled->signal == &source);
    CHECK(!g.sampled->next && g.sampled->history);
    llg_sampled_register_value(&source);
    CHECK(!g.sampled_values);
    sv4_t copy = SV4_EMPTY;
    CHECK(llg_sampled_copy(&source, &copy));
    expect_number(copy, 5);
    llg_rt_cleanup();
    sv4_destroy(&source);
    CHECK(value_test_live() == 0);
}

int main(void) {
    completed_spawns_are_reclaimed();
    completed_parent_waits_for_join_none_child();
    sampled_domain_history_is_bounded();
    clocking_history_is_bounded();
    value_registration_is_promoted();
    puts("runtime retention probe passed");
    return 0;
}
