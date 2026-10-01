#include "llg_rt.c"
#include <time.h>
#include "probe.h"
#include "probe_co.h"

#define GROUP_COUNT 100000u
#define SCALE_REPEATS 3u

static unsigned count;
static unsigned started;
static unsigned completed;
static int cancel_children;
static int one_group;
static int finish_parked;
static int staggered;
static int mixed_completion;
static unsigned quick_completed;
static llg_event_object_t park_object;
static llg_event_t park = {&park_object};

typedef struct {
    llg_co_frame_t co;
    unsigned index;
} child_frame_t;
LLG_CO_ROOT_FRAME_OK(child_frame_t);

LLG_PROBE_PROCESS(scale_child, child_frame_t, 1) {
    LLG_PROBE_BEGIN(child_frame_t, 1);
    CHECK(F->index == started++);
    LLG_PROBE_AWAIT(1, llg_arm_event(self, &park));
    CHECK(!cancel_children);
    completed++;
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(quick_child, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    quick_completed++;
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(scale_parent, 4) {
    LLG_PROBE_SIMPLE_BEGIN(4);
    llg_fork_group_t* group = NULL;
    for (F->index = 0; (unsigned)F->index < count; F->index++) {
        if (!one_group || !group) group = llg_fork_group_new(LLG_JOIN_NONE);
        llg_proc_t* child = llg_fork(&scale_child_desc, "ordered child", group);
        ((child_frame_t*)LLG_CO_ROOT(&child->chain))->index = (unsigned)F->index;
        if (!one_group) CHECK(llg_arm_join(self, group) == LLG_CO_ARM_READY);
        if (staggered) {
            LLG_PROBE_AWAIT(3, llg_arm_time(self, 0));
            CHECK(started == (unsigned)F->index + 1);
        }
    }
    if (one_group) CHECK(llg_arm_join(self, group) == LLG_CO_ARM_READY);
    CHECK(started == (staggered ? count : 0));
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    CHECK(started == count && completed == 0);
    if (mixed_completion) {
        for (unsigned i = 0; i < count; i++) {
            llg_fork_group_t* quick_group = llg_fork_group_new(LLG_JOIN_NONE);
            llg_fork(&quick_child_desc, "quick child", quick_group);
            CHECK(llg_arm_join(self, quick_group) == LLG_CO_ARM_READY);
        }
        LLG_PROBE_AWAIT(4, llg_arm_time(self, 1));
        CHECK(quick_completed == count);
    }
    if (finish_parked) {
        llg_rt_request_finish();
        LLG_PROBE_EXIT();
    } else if (cancel_children) {
        llg_disable_fork(self);
        CHECK(!self->fork_groups && !self->fork_groups_tail);
        CHECK(!self->pending_fork_groups && !park.object->n_waiters);
        CHECK(llg_arm_wait_fork(self) == LLG_CO_ARM_READY);
    } else {
        llg_event_trigger(&park);
        LLG_PROBE_AWAIT(2, llg_arm_wait_fork(self));
        CHECK(completed == count && !self->fork_groups);
        CHECK(!self->fork_groups_tail && !self->pending_fork_groups);
    }
    LLG_PROBE_DONE();
}

static double run(unsigned n, int cancel, int single) {
    count = n;
    started = completed = quick_completed = 0;
    mixed_completion = cancel == 4;
    cancel_children = mixed_completion ? 0 : cancel;
    finish_parked = cancel == 2 || cancel == 3;
    staggered = cancel == 3;
    one_group = single;
    clock_t begin = clock();
    llg_rt_init();
    llg_event_object_reset(&park_object);
    llg_spawn(&scale_parent_desc, "parent");
    llg_rt_run();
    CHECK(started == n && completed == (cancel_children ? 0 : n));
    llg_rt_cleanup();
    CHECK(!g.all_procs && !g.zombie_groups && !g.waiters && !g.process_handles);
    llg_event_object_reset(&park_object);
    CHECK(value_test_live() == 0);
    return (double)(clock() - begin) / CLOCKS_PER_SEC;
}

static double removal_order(unsigned n) {
    clock_t begin = clock();
    llg_rt_init();
    llg_proc_t* parent = llg_spawn(&scale_parent_desc, "unlink parent");
    g.current = parent;
    llg_fork_group_t** groups = llg_checked_malloc(
        n, sizeof(*groups), "probe groups");
    for (unsigned i = 0; i < n; i++)
        groups[i] = llg_fork_group_new(LLG_JOIN_NONE);
    // Reverse completion exercises tail removal rather than only the head.
    for (unsigned i = n; i > 0; i--) {
        CHECK(parent->fork_groups_tail == groups[i - 1]);
        CHECK(llg_arm_join(parent, groups[i - 1]) == LLG_CO_ARM_READY);
    }
    CHECK(!parent->fork_groups && !parent->fork_groups_tail);
    CHECK(!parent->pending_fork_groups);
    free(groups);
    g.current = NULL;
    llg_rt_cleanup();
    return (double)(clock() - begin) / CLOCKS_PER_SEC;
}

static void event_positions(void) {
    for (int triggered = 0; triggered < 2; triggered++) {
        llg_rt_init();
        llg_event_object_reset(&park_object);
        llg_proc_t* procs[4];
        for (unsigned i = 0; i < 4; i++) {
            procs[i] = llg_spawn(&scale_child_desc, "indexed waiter");
            remove_region_entry(procs[i]);
            g.current = procs[i];
            llg_co_arm_t arm = triggered
                ? llg_arm_event_triggered(procs[i], &park)
                : llg_arm_event(procs[i], &park);
            CHECK(arm == LLG_CO_ARM_SUSPEND);
        }
        g.current = NULL;
        llg_kill_proc_tree(procs[1]);
        llg_kill_proc_tree(procs[0]);
        llg_proc_t** table = triggered ? park_object.triggered_waiters : park_object.waiters;
        CHECK(table[0] == procs[2] && table[1] == procs[3]);
        llg_event_trigger(&park);
        CHECK(!park_object.n_waiters && !park_object.n_triggered_waiters);
        CHECK(procs[2]->wait.kind == W_NONE && procs[3]->wait.kind == W_NONE);
        llg_rt_cleanup();
        llg_event_object_reset(&park_object);
    }
    llg_rt_init();
    llg_proc_t* duplicate = llg_spawn(&scale_child_desc, "duplicate event waiter");
    remove_region_entry(duplicate);
    g.current = duplicate;
    const llg_event_t* events[] = {&park, &park};
    CHECK(llg_arm_events(duplicate, events, 2) == LLG_CO_ARM_SUSPEND);
    g.current = NULL;
    CHECK(park_object.n_waiters == 2);
    llg_kill_proc_tree(duplicate);
    CHECK(park_object.n_waiters == 0);
    llg_rt_cleanup();
    llg_event_object_reset(&park_object);
}

int main(void) {
    event_positions();
    double a = removal_order(GROUP_COUNT);
    double b = removal_order(2u * GROUP_COUNT);
    printf("reverse removal N=%.6f 2N=%.6f ratio=%.3f\n", a, b, b / a);
    CHECK(a > 0 && b < 3.6 * a);
    for (int mode = 0; mode < 6; mode++) {
        double small = 1e30;
        double large = 1e30;
        for (unsigned repeat = 0; repeat < SCALE_REPEATS; repeat++) {
            double a = run(GROUP_COUNT, mode >= 3 ? mode - 1 : mode != 0, mode == 2);
            double b = run(2u * GROUP_COUNT, mode >= 3 ? mode - 1 : mode != 0, mode == 2);
            if (a < small) small = a;
            if (b < large) large = b;
        }
        printf("fork scale mode=%d N=%u cpu=%.6f 2N=%.6f ratio=%.3f\n",
               mode, GROUP_COUNT, small, large, large / small);
        CHECK(small > 0 && large < 3.6 * small);
    }
    return 0;
}
