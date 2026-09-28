/* Focused Phase 4 scheduler and stackless-chain regressions. */
#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static int deep_leaf_entered;
static int deep_returned;
static llg_proc_t* deep_victim;

typedef struct {
    llg_co_frame_t co;
} deep_leaf_frame_t;
LLG_CO_ROOT_FRAME_OK(deep_leaf_frame_t);

LLG_PROBE_PROCESS(deep_leaf, deep_leaf_frame_t, 1) {
    LLG_PROBE_BEGIN(deep_leaf_frame_t, 1);
    deep_leaf_entered++;
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 100));
    deep_returned++;
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    llg_co_anchor_t* slot;
} deep_anchor_frame_t;
LLG_CO_ROOT_FRAME_OK(deep_anchor_frame_t);

LLG_PROBE_PROCESS(deep_anchor, deep_anchor_frame_t, 1) {
    LLG_PROBE_BEGIN(deep_anchor_frame_t, 1);
    LLG_CO_ARENA_ENTER(ch, &deep_leaf_desc, F->slot);
    LLG_CO_CALL_ARENA(co, ch, 1, &deep_leaf_desc, F->slot);
    deep_returned++;
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    LLG_CO_ANCHORED(deep_anchor_frame_t) child;
} deep_polled_frame_t;
LLG_CO_ROOT_FRAME_OK(deep_polled_frame_t);
LLG_CO_ANCHORED_OK(deep_anchor_frame_t);

LLG_PROBE_PROCESS(deep_polled, deep_polled_frame_t, 1) {
    LLG_PROBE_BEGIN(deep_polled_frame_t, 1);
    LLG_CO_CALL_ANCHOR(co, ch, 1, &deep_anchor_desc, &F->child.an);
    deep_returned++;
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    deep_polled_frame_t child;
} deep_root_frame_t;
LLG_CO_ROOT_FRAME_OK(deep_root_frame_t);

LLG_PROBE_PROCESS(deep_root, deep_root_frame_t, 1) {
    LLG_PROBE_BEGIN(deep_root_frame_t, 1);
    (void)&deep_polled_desc;
    LLG_CO_CALL(co, ch, 1, deep_polled, (llg_co_frame_t*)&F->child);
    deep_returned++;
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(deep_killer, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    llg_kill_proc_tree(deep_victim);
    deep_victim = NULL;
    llg_rt_request_finish();
    LLG_PROBE_EXIT();
}

static void check_deep_cancellation(void) {
    llg_rt_init();
    deep_leaf_entered = 0;
    deep_returned = 0;
    deep_victim = llg_spawn(&deep_root_desc, "deep cancellation victim");
    llg_spawn(&deep_killer_desc, "deep cancellation killer");
    llg_rt_run();
    CHECK(deep_leaf_entered == 1);
    CHECK(deep_returned == 0);
    CHECK(g.retired_procs == NULL);
    llg_rt_cleanup();
}

static int detached_child_runs;

LLG_PROBE_SIMPLE_PROCESS(detached_child, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    detached_child_runs++;
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
} immediate_call_frame_t;
LLG_CO_ROOT_FRAME_OK(immediate_call_frame_t);

LLG_PROBE_PROCESS(immediate_call, immediate_call_frame_t, 0) {
    LLG_PROBE_BEGIN(immediate_call_frame_t, 0);
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    llg_fork_group_t* group;
    immediate_call_frame_t call;
} join_none_frame_t;
LLG_CO_ROOT_FRAME_OK(join_none_frame_t);

LLG_PROBE_PROCESS(join_none_parent, join_none_frame_t, 3) {
    LLG_PROBE_BEGIN(join_none_frame_t, 3);
    (void)&immediate_call_desc;
    F->group = llg_fork_group_new(LLG_JOIN_NONE);
    llg_fork(&detached_child_desc, "detached child", F->group);
    LLG_CO_CALL(co, ch, 1, immediate_call, (llg_co_frame_t*)&F->call);
    CHECK(detached_child_runs == 0);
    LLG_PROBE_AWAIT(2, llg_arm_join(self, F->group));
    CHECK(detached_child_runs == 0);
    LLG_PROBE_AWAIT(3, llg_arm_time(self, 1));
    CHECK(detached_child_runs == 1);
    llg_rt_request_finish();
    LLG_PROBE_EXIT();
}

static void check_join_none_boundaries(void) {
    llg_rt_init();
    detached_child_runs = 0;
    llg_spawn(&join_none_parent_desc, "join_none boundary parent");
    llg_rt_run();
    CHECK(detached_child_runs == 1);
    llg_rt_cleanup();
}

static int stop_stage;
static int deferred_follower_ran;

LLG_PROBE_SIMPLE_PROCESS(exact_stop, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    stop_stage = 1;
    LLG_PROBE_AWAIT(1, llg_arm_stop(self, 0, "stackless probe exact stop"));
    stop_stage = 2;
    LLG_PROBE_DONE();
}

static void request_deferred_stop(void) {
    llg_rt_request_stop(0, "stackless probe deferred stop");
}

LLG_PROBE_SIMPLE_PROCESS(deferred_stop, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    stop_stage = 1;
    request_deferred_stop();
    stop_stage = 2;
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(deferred_follower, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    deferred_follower_ran++;
    LLG_PROBE_DONE();
}

static void check_stop_paths(void) {
    CHECK(llg_rt_set_stop_policy(LLG_STOP_POLICY_EXIT));
    llg_rt_init();
    stop_stage = 0;
    llg_spawn(&exact_stop_desc, "exact stop");
    llg_rt_run();
    CHECK(llg_rt_is_suspended() && stop_stage == 1);
    CHECK(llg_rt_resume());
    llg_rt_run();
    CHECK(!llg_rt_is_suspended() && stop_stage == 2);
    llg_rt_cleanup();

    llg_rt_init();
    stop_stage = 0;
    deferred_follower_ran = 0;
    llg_spawn(&deferred_stop_desc, "deferred stop");
    llg_spawn(&deferred_follower_desc, "deferred follower");
    llg_rt_run();
    CHECK(llg_rt_is_suspended() && stop_stage == 2);
    CHECK(deferred_follower_ran == 0);
    CHECK(llg_rt_resume());
    llg_rt_run();
    CHECK(!llg_rt_is_suspended() && deferred_follower_ran == 1);
    llg_rt_cleanup();
    CHECK(llg_rt_set_stop_policy(LLG_STOP_POLICY_RESUME));
}

static int final_count;

static void finishing_final(void) {
    CHECK(llg_current() != NULL);
    final_count++;
    llg_rt_finish();
}

static void skipped_final(void) { final_count += 100; }

static void check_final_finish(void) {
    llg_rt_init();
    final_count = 0;
    llg_spawn_final(finishing_final, "finishing final");
    llg_spawn_final(skipped_final, "skipped final");
    llg_rt_run();
    llg_rt_run_finals();
    CHECK(final_count == 1);
    CHECK(llg_current() == NULL);
}

static int budget_points;

LLG_PROBE_SIMPLE_PROCESS(budget_abort_process, 0) {
    LLG_PROBE_SIMPLE_BEGIN(0);
    for (;;) {
        if (llg_budget_point("stackless-runtime-probe:budget"))
            LLG_PROBE_EXIT();
        budget_points++;
    }
}

static void check_budget_abort(void) {
    llg_rt_init();
    g.process_step_limit = 3;
    budget_points = 0;
    llg_spawn(&budget_abort_process_desc, "budget abort");
    llg_rt_run();
    CHECK(llg_rt_failed());
    CHECK(budget_points == 3);
    llg_rt_cleanup();
}

static llg_semaphore_t* semaphore;
static llg_process_handle_t* semaphore_head_handle;
static sv4_t semaphore_zero = SV4_EMPTY;
static sv4_t semaphore_one = SV4_EMPTY;
static int semaphore_head_resumed;
static int semaphore_tail_resumed;

LLG_PROBE_SIMPLE_PROCESS(semaphore_head, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_semaphore_get(self, semaphore, semaphore_one));
    semaphore_head_resumed++;
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(semaphore_tail, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_semaphore_get(self, semaphore, semaphore_one));
    semaphore_tail_resumed++;
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(semaphore_killer, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    llg_process_kill(self, semaphore_head_handle);
    llg_semaphore_put(semaphore, semaphore_one);
    LLG_PROBE_DONE();
}

static void check_semaphore_head_cancellation(void) {
    llg_rt_init();
    semaphore_zero = sv4_from_u64(0, 32, 0);
    semaphore_one = sv4_from_u64(1, 32, 0);
    semaphore = llg_semaphore_new(semaphore_zero);
    semaphore_head_resumed = 0;
    semaphore_tail_resumed = 0;
    llg_proc_t* head = llg_spawn(&semaphore_head_desc, "semaphore head");
    semaphore_head_handle = head->handle;
    llg_process_retain(semaphore_head_handle);
    llg_spawn(&semaphore_tail_desc, "semaphore tail");
    llg_spawn(&semaphore_killer_desc, "semaphore killer");
    llg_rt_run();
    CHECK(semaphore_head_resumed == 0);
    CHECK(semaphore_tail_resumed == 1);
    CHECK(llg_process_status(semaphore_head_handle) == LLG_PROCESS_KILLED);
    llg_process_release(semaphore_head_handle);
    semaphore_head_handle = NULL;
    llg_rt_cleanup();
    sv4_destroy(&semaphore_zero);
    sv4_destroy(&semaphore_one);
}

static llg_mailbox_t* mismatch_mailbox;
static sv4_t mismatch_target = SV4_EMPTY;
static int mismatch_continued;

LLG_PROBE_SIMPLE_PROCESS(mailbox_mismatch, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(
        1, llg_arm_mailbox_get_value(
               self, mismatch_mailbox,
               llg_mailbox_target_packed(&mismatch_target, 8, 0, 0), 0));
    mismatch_continued++;
    LLG_PROBE_DONE();
}

static void check_mailbox_complete_exit(void) {
    llg_rt_init();
    sv4_t bound = sv4_from_u64(0, 32, 0);
    mismatch_mailbox =
        llg_mailbox_new(bound, LLG_MAILBOX_UNTYPED, 0, 0, 0, 0);
    sv4_destroy(&bound);
    CHECK(llg_mailbox_try_put_value(
        mismatch_mailbox, llg_mailbox_value_real(3.5, 0)));
    sv4_replace(&mismatch_target, sv4_zero(8, 0));
    mismatch_continued = 0;
    llg_proc_t* proc = llg_spawn(&mailbox_mismatch_desc, "mailbox mismatch");
    llg_process_handle_t* handle = proc->handle;
    llg_process_retain(handle);
    llg_rt_run();
    CHECK(llg_rt_failed());
    CHECK(mismatch_continued == 0);
    CHECK(llg_process_status(handle) == LLG_PROCESS_FINISHED);
    llg_process_release(handle);
    sv4_destroy(&mismatch_target);
    llg_rt_cleanup();
}

typedef struct {
    llg_co_frame_t co;
} arena_counter_leaf_frame_t;
LLG_CO_ROOT_FRAME_OK(arena_counter_leaf_frame_t);

LLG_PROBE_PROCESS(arena_counter_leaf, arena_counter_leaf_frame_t, 0) {
    LLG_PROBE_BEGIN(arena_counter_leaf_frame_t, 0);
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    llg_co_anchor_t* slot;
} arena_counter_root_frame_t;
LLG_CO_ROOT_FRAME_OK(arena_counter_root_frame_t);

static llg_rt_co_cache_stats_t arena_first_stats;
static llg_rt_co_cache_stats_t arena_second_stats;
static llg_rt_co_cache_stats_t arena_before_stats;
static int arena_idle_checks;

LLG_PROBE_PROCESS(arena_counter_root, arena_counter_root_frame_t, 3) {
    LLG_PROBE_BEGIN(arena_counter_root_frame_t, 3);
    LLG_CO_ARENA_ENTER(ch, &arena_counter_leaf_desc, F->slot);
    LLG_CO_CALL_ARENA(co, ch, 1, &arena_counter_leaf_desc, F->slot);
    CHECK(ch->arena.head == NULL);
    arena_idle_checks++;
    llg_rt_co_cache_get_stats(&arena_first_stats);
    LLG_CO_ARENA_ENTER(ch, &arena_counter_leaf_desc, F->slot);
    LLG_CO_CALL_ARENA(co, ch, 2, &arena_counter_leaf_desc, F->slot);
    CHECK(ch->arena.head == NULL);
    arena_idle_checks++;
    llg_rt_co_cache_get_stats(&arena_second_stats);
    LLG_PROBE_AWAIT(3, llg_arm_time(self, 1));
    CHECK(ch->arena.head == NULL);
    llg_rt_request_finish();
    LLG_PROBE_EXIT();
}

static void check_arena_counters(void) {
    llg_rt_init();
    arena_idle_checks = 0;
    llg_rt_co_cache_get_stats(&arena_before_stats);
    memset(&arena_first_stats, 0, sizeof(arena_first_stats));
    memset(&arena_second_stats, 0, sizeof(arena_second_stats));
    llg_spawn(&arena_counter_root_desc, "arena counter root");
    llg_rt_run();
    CHECK(arena_idle_checks == 2);
    CHECK(arena_first_stats.system_allocations ==
          arena_before_stats.system_allocations + 1);
    CHECK(arena_second_stats.system_allocations ==
          arena_first_stats.system_allocations);
    CHECK(arena_second_stats.cache_hits == arena_first_stats.cache_hits + 1);
    CHECK(arena_second_stats.cached_bytes == arena_first_stats.cached_bytes);
    llg_rt_cleanup();
    llg_rt_co_cache_stats_t after_cleanup;
    llg_rt_co_cache_get_stats(&after_cleanup);
    CHECK(after_cleanup.cached_bytes == 0);
}

int main(void) {
    check_deep_cancellation();
    check_join_none_boundaries();
    check_stop_paths();
    check_final_finish();
    check_budget_abort();
    check_semaphore_head_cancellation();
    check_mailbox_complete_exit();
    check_arena_counters();
    puts("stackless runtime boundaries: OK");
    return 0;
}
