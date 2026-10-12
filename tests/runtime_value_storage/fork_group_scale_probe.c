#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"
#include <string.h>

#define GROUP_COUNT 100000u
/* Removal alone finishes in ~10 ms at GROUP_COUNT, where scheduler jitter is a
 * large fraction of the sample, so it runs on a proportionally larger
 * population. */
#define REMOVAL_COUNT (4u * GROUP_COUNT)
/* Doubling the population may cost at most this factor in CPU time. Linear
 * scaling measures about 2-3 (cache growth included); quadratic list walks
 * give about 4. */
#define SCALE_LIMIT 3.6
/* Doubling the population costs at least this factor: every case does work
 * proportional to N beyond a small fixed setup. A lower ratio means the CPU
 * clock did not resolve the samples (a coarse clock read 1.000 and 4.000). */
#define SCALE_MIN_RATIO 1.3
/* The minima over interleaved small/large measurements only improve as rounds
 * accumulate. A case passes as soon as its minima satisfy the limit after
 * SCALE_MIN_ROUNDS; contention noise is rejected by re-measuring up to
 * SCALE_MAX_ROUNDS or SCALE_BUDGET_SECONDS of measured CPU time, and only a
 * ratio that stays at or above the limit fails. */
#define SCALE_MIN_ROUNDS 3u
#define SCALE_MAX_ROUNDS 16u
#define SCALE_BUDGET_SECONDS 20.0
/* Each sample repeats its measurement until it spans this many ticks of the
 * CPU clock, so one tick of quantization moves a sample by at most 1/32 and a
 * ratio by at most about 6%: a quadratic 4 still reads above SCALE_LIMIT and
 * a linear 3 below it. Windows process CPU time ticks every 15.625 ms, where a
 * single removal sample (one tick) gave ratio 4.000; clock() on Linux and
 * macOS ticks every microsecond, so one run per sample suffices there. */
#define SCALE_RESOLUTION_MULTIPLE 32.0

/* Process CPU time (probe_cpu_time.c): the UCRT's clock() is wall time, which
 * counts time other processes run and so inflates both the ratios and the
 * budget under a parallel test load. */
double probe_cpu_seconds(void);

/* The smallest step of probe_cpu_seconds observed over a few ticks while this
 * process spins, i.e. the clock's resolution on this host. */
static double cpu_clock_resolution(void) {
    double resolution = 1e30;
    double last = probe_cpu_seconds();
    CHECK(last >= 0);
    for (unsigned ticks = 0; ticks < 4u;) {
        double now = probe_cpu_seconds();
        if (now > last) {
            if (now - last < resolution) resolution = now - last;
            last = now;
            ++ticks;
        }
    }
    return resolution;
}

static double min_sample_seconds;

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

typedef struct {
    llg_co_frame_t co;
    int index;
    llg_fork_group_t* group;
} parent_frame_t;
LLG_CO_ROOT_FRAME_OK(parent_frame_t);

LLG_PROBE_PROCESS(scale_parent, parent_frame_t, 4) {
    LLG_PROBE_BEGIN(parent_frame_t, 4);
    F->group = NULL;
    for (F->index = 0; (unsigned)F->index < count; F->index++) {
        if (!one_group || !F->group) F->group = llg_fork_group_new(LLG_JOIN_NONE);
        llg_proc_t* child = llg_fork(&scale_child_desc, "ordered child", F->group);
        ((child_frame_t*)LLG_CO_ROOT(&child->chain))->index = (unsigned)F->index;
        if (!one_group) CHECK(llg_arm_join(self, F->group) == LLG_CO_ARM_READY);
        if (staggered) {
            LLG_PROBE_AWAIT(3, llg_arm_time(self, 0));
            CHECK(started == (unsigned)F->index + 1);
        }
    }
    if (one_group) CHECK(llg_arm_join(self, F->group) == LLG_CO_ARM_READY);
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

typedef struct {
    int cancel;
    int single;
} fork_mode_t;

static double run(unsigned n, fork_mode_t mode) {
    int cancel = mode.cancel;
    count = n;
    started = completed = quick_completed = 0;
    mixed_completion = cancel == 4;
    cancel_children = mixed_completion ? 0 : cancel;
    finish_parked = cancel == 2 || cancel == 3;
    staggered = cancel == 3;
    one_group = mode.single;
    double begin = probe_cpu_seconds();
    llg_rt_init();
    llg_event_object_reset(&park_object);
    llg_spawn(&scale_parent_desc, "parent");
    llg_rt_run();
    CHECK(started == n && completed == (cancel_children ? 0 : n));
    llg_rt_cleanup();
    CHECK(!g.all_procs && !g.zombie_groups && !g.waiters && !g.process_handles);
    llg_event_object_reset(&park_object);
    CHECK(value_test_live() == 0);
    return probe_cpu_seconds() - begin;
}

static double removal_order(unsigned n, fork_mode_t unused) {
    (void)unused;
    double begin = probe_cpu_seconds();
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
    return probe_cpu_seconds() - begin;
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

/* CPU time of one `measure(n, mode)`: the mean over as many back-to-back runs
 * as fill min_sample_seconds, timed as one span. */
static double sample(double (*measure)(unsigned, fork_mode_t), unsigned n, fork_mode_t mode) {
    double begin = probe_cpu_seconds(), spent;
    unsigned runs = 0;
    do {
        (void)measure(n, mode);
        ++runs;
        spent = probe_cpu_seconds() - begin;
    } while (spent < min_sample_seconds);
    return spent / runs;
}

/* Measures `measure` at n and 2n, alternating which size goes first so a
 * burst of load does not systematically hit one of them. Returns whether the
 * minima's ratio lies between SCALE_MIN_RATIO and SCALE_LIMIT; *small and
 * *large receive the minima. */
static int scales_linearly(double (*measure)(unsigned, fork_mode_t), unsigned n,
                           fork_mode_t mode, double* small, double* large, unsigned* rounds) {
    double begin = probe_cpu_seconds();
    *small = *large = 1e30;
    for (*rounds = 0; *rounds < SCALE_MAX_ROUNDS &&
                      probe_cpu_seconds() - begin < SCALE_BUDGET_SECONDS;) {
        double a, b;
        if (*rounds % 2) {
            b = sample(measure, 2u * n, mode);
            a = sample(measure, n, mode);
        } else {
            a = sample(measure, n, mode);
            b = sample(measure, 2u * n, mode);
        }
        ++*rounds;
        if (a < *small) *small = a;
        if (b < *large) *large = b;
        if (*rounds >= SCALE_MIN_ROUNDS && *small > 0 && *large < SCALE_LIMIT * *small &&
            *large > SCALE_MIN_RATIO * *small)
            return 1;
    }
    return 0;
}

/* A scaling failure is a timing verdict, not an ownership check. */
static void require_linear(int ok, const char* name) {
    if (ok) return;
    fprintf(stderr,
            "%s: the 2N/N CPU-time ratio above stayed outside (%.1f, %.1f): at or above the "
            "upper bound is superlinear growth, at or below the lower one an unresolved clock\n",
            name, SCALE_MIN_RATIO, SCALE_LIMIT);
    exit(2);
}

/* `removal` checks only the reverse-removal case (the coarse-clock test). */
int main(int argc, char** argv) {
    int removal_only = argc > 1 && strcmp(argv[1], "removal") == 0;
    event_positions();
    double resolution = cpu_clock_resolution();
    min_sample_seconds = SCALE_RESOLUTION_MULTIPLE * resolution;
    printf("cpu clock resolution=%.6f min sample=%.6f\n", resolution, min_sample_seconds);
    double small, large;
    unsigned rounds;
    int ok = scales_linearly(removal_order, REMOVAL_COUNT, (fork_mode_t){0, 0}, &small, &large, &rounds);
    printf("reverse removal N=%u cpu=%.6f 2N=%.6f ratio=%.3f rounds=%u\n",
           REMOVAL_COUNT, small, large, large / small, rounds);
    require_linear(ok, "reverse removal");
    for (int mode = 0; mode < 6 && !removal_only; mode++) {
        fork_mode_t shape = {mode >= 3 ? mode - 1 : mode != 0, mode == 2};
        ok = scales_linearly(run, GROUP_COUNT, shape, &small, &large, &rounds);
        printf("fork scale mode=%d N=%u cpu=%.6f 2N=%.6f ratio=%.3f rounds=%u\n",
               mode, GROUP_COUNT, small, large, large / small, rounds);
        require_linear(ok, "fork scale");
    }
    return 0;
}
