#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static const char* mode;
static llg_proc_t* cancel_victim;
static unsigned arena_completed;

typedef struct {
    llg_co_frame_t co;
    unsigned char payload[32];
} leaf_frame_t;

LLG_PROBE_PROCESS(debug_leaf, leaf_frame_t, 1) {
    LLG_PROBE_BEGIN(leaf_frame_t, 1);
    LLG_CO_SUSPEND(co, ch, 1);
    LLG_PROBE_DONE();
}

LLG_PROBE_PROCESS(cancel_leaf, leaf_frame_t, 1) {
    LLG_PROBE_BEGIN(leaf_frame_t, 1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 2));
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    unsigned char payload[32];
    leaf_frame_t child;
    LLG_CO_ANCHORED(leaf_frame_t) anchor;
    llg_co_anchor_t* slot;
} root_frame_t;
LLG_CO_ROOT_FRAME_OK(root_frame_t);
LLG_CO_ANCHORED_OK(leaf_frame_t);

LLG_PROBE_PROCESS(debug_root, root_frame_t, 3) {
    LLG_PROBE_BEGIN(root_frame_t, 3);
    if (!strcmp(mode, "reentrant")) return llg_co_run(ch);
    if (!strcmp(mode, "exiting-service")) {
        llg_rt_finish();
        (void)llg_arm_time(self, 1);
    }
    if (!strncmp(mode, "static-", 7)) {
        memset(&F->child, 0, sizeof(F->child));
        if (!strcmp(mode, "static-enter")) F->child.co.flags = LLG_CO_FRAME_LIVE;
        LLG_CO_CALL(co, ch, 1, debug_leaf, &F->child.co);
    }
    if (!strncmp(mode, "anchor-", 7) || !strcmp(mode, "anchored-root-tag")) {
        memset(&F->anchor, 0, sizeof(F->anchor));
        if (!strcmp(mode, "anchor-enter")) F->anchor.f.co.flags = LLG_CO_FRAME_LIVE;
        LLG_CO_CALL_ANCHOR(co, ch, 2, &debug_leaf_desc, &F->anchor.an);
    }
    if (!strcmp(mode, "cancelled-arena-reuse")) {
        LLG_CO_ARENA_ENTER(ch, &cancel_leaf_desc, F->slot);
        LLG_CO_CALL_ARENA(co, ch, 3, &cancel_leaf_desc, F->slot);
        arena_completed++;
    }
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(cancel_driver, 2) {
    LLG_PROBE_SIMPLE_BEGIN(2);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    llg_kill_proc_tree(cancel_victim);
    cancel_victim = NULL;
    llg_spawn(&debug_root_desc, "reused cancelled arena");
    LLG_PROBE_AWAIT(2, llg_arm_time(self, 3));
    llg_rt_request_finish();
    LLG_PROBE_EXIT();
}

static void check_poison(const llg_proc_t* proc) {
    const root_frame_t* frame = (const root_frame_t*)LLG_CO_ROOT(&proc->chain);
    CHECK(frame->co.state == 0 && frame->co.flags == LLG_CO_FRAME_LIVE);
    for (size_t i = sizeof(llg_co_frame_t); i < sizeof(*frame); i++)
        CHECK(((const unsigned char*)frame)[i] == 0xA5);
}

int main(int argc, char** argv) {
    CHECK(argc == 2);
    mode = argv[1];
    llg_rt_init();
    llg_proc_t* proc = llg_spawn(&debug_root_desc, "debug misuse root");
    CHECK(proc != NULL);
    if (!strcmp(mode, "cancelled-arena-reuse")) {
        cancel_victim = proc;
        llg_spawn(&cancel_driver_desc, "arena cancellation driver");
        llg_rt_run();
        CHECK(arena_completed == 1);
        llg_rt_co_cache_stats_t stats;
        llg_rt_co_cache_get_stats(&stats);
        CHECK(stats.system_allocations == 1 && stats.cache_hits == 1);
        llg_rt_cleanup();
        puts("cancelled arena reused: OK");
        return 0;
    }
    if (!strcmp(mode, "fresh-poison")) {
        check_poison(proc);
        g.current = proc;
        llg_fork_group_t* group = llg_fork_group_new(LLG_JOIN_NONE);
        llg_proc_t* child = llg_fork(&debug_root_desc, "poisoned fork", group);
        CHECK(child != NULL);
        check_poison(child);
        g.current = NULL;
        llg_rt_cleanup();
        puts("fresh spawn and fork roots poisoned: OK");
        return 0;
    }
    g.current = proc;
    root_frame_t* frame = (root_frame_t*)LLG_CO_ROOT(&proc->chain);
    if (!strcmp(mode, "root-tag")) frame->co.flags = 0;
    if (!strcmp(mode, "invalid-state")) frame->co.state = UINT32_MAX;
    if (!strcmp(mode, "poison-state"))
        memset(&frame->co.state, 0xA5, sizeof(frame->co.state));
    llg_co_status_t status = llg_co_run(&proc->chain);
    if (!strcmp(mode, "finished")) {
        CHECK(status == LLG_CO_DONE);
        (void)llg_co_run(&proc->chain);
    }
    if (!strcmp(mode, "static-tag")) {
        CHECK(status == LLG_CO_PENDING);
        frame->child.co.flags = 0xA5A5A5A5u;
        (void)llg_co_run(&proc->chain);
    }
    if (!strcmp(mode, "anchor-tag")) {
        CHECK(status == LLG_CO_PENDING);
        frame->anchor.f.co.flags = 0xA5A5A5A5u;
        (void)llg_co_run(&proc->chain);
    }
    if (!strcmp(mode, "anchored-root-tag")) {
        CHECK(status == LLG_CO_PENDING && proc->chain.top != NULL);
        frame->co.flags = 0;
        (void)llg_co_run(&proc->chain);
    }
    fprintf(stderr, "debug misuse unexpectedly returned: %s\n", mode);
    return 0;
}
