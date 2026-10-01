#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

#define SCALE_PROCESSES 32
#define SCALE_ROUNDS 8
#define SCALE_TRANSIENT_BYTES (8u * 1024u * 1024u)

static llg_proc_t* workers[SCALE_PROCESSES];
static llg_rt_co_cache_stats_t warmed;
static unsigned snapshots;
static unsigned completions;

LLG_PROBE_SIMPLE_PROCESS(scale_leaf, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    llg_co_anchor_t* slot;
    unsigned round;
} scale_frame_t;
LLG_CO_ROOT_FRAME_OK(scale_frame_t);

LLG_PROBE_PROCESS(scale_worker, scale_frame_t, 2) {
    LLG_PROBE_BEGIN(scale_frame_t, 2);
    for (F->round = 0; F->round < SCALE_ROUNDS; F->round++) {
        LLG_CO_ARENA_ENTER(ch, &scale_leaf_desc, F->slot);
        LLG_CO_CALL_ARENA(co, ch, 1, &scale_leaf_desc, F->slot);
        CHECK(ch->arena.head == NULL && F->slot == NULL && ch->top == NULL);
        completions++;
        LLG_PROBE_AWAIT(2, llg_arm_time(self, 2));
    }
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(scale_observer, 2) {
    LLG_PROBE_SIMPLE_BEGIN(2);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 2));
    for (F->index = 0; F->index < SCALE_ROUNDS; F->index++) {
        llg_rt_co_cache_stats_t stats;
        llg_rt_co_cache_get_stats(&stats);
        CHECK(completions == SCALE_PROCESSES * (unsigned)(F->index + 1));
        for (unsigned i = 0; i < SCALE_PROCESSES; i++) {
            CHECK(workers[i]->wait.kind == W_TIME);
            CHECK(workers[i]->chain.arena.head == NULL);
            CHECK(workers[i]->chain.top == NULL);
        }
        if (!F->index) {
            warmed = stats;
            CHECK(warmed.system_allocations == SCALE_PROCESSES + 1);
            CHECK(warmed.cached_bytes == SCALE_PROCESSES *
                  (sizeof(llg_co_chunk_t) + LLG_CO_ARENA_MIN_CHUNK));
        } else {
            CHECK(stats.system_allocations == warmed.system_allocations);
            CHECK(stats.system_frees == warmed.system_frees);
            CHECK(stats.cache_hits == warmed.cache_hits +
                  SCALE_PROCESSES * (unsigned)F->index);
            CHECK(stats.cached_bytes == warmed.cached_bytes);
        }
        snapshots++;
        if (F->index + 1 < SCALE_ROUNDS) {
            LLG_PROBE_AWAIT(2, llg_arm_time(self, 3));
        }
    }
    llg_rt_request_finish();
    LLG_PROBE_EXIT();
}

int main(void) {
    llg_rt_init();
    for (unsigned i = 0; i < SCALE_PROCESSES; i++)
        workers[i] = llg_spawn(&scale_worker_desc, "scale worker");
    void* transient = llg_co_arena_push(&workers[0]->chain.arena, SCALE_TRANSIENT_BYTES);
    CHECK(transient != NULL && workers[0]->chain.arena.head != NULL);
    CHECK(workers[0]->chain.arena.head->size >= SCALE_TRANSIENT_BYTES);
    llg_co_arena_pop(&workers[0]->chain.arena, transient);
    CHECK(workers[0]->chain.arena.head == NULL);
    llg_spawn(&scale_observer_desc, "scale observer");
    llg_rt_run();
    CHECK(snapshots == SCALE_ROUNDS);
    llg_rt_co_cache_stats_t stats;
    llg_rt_co_cache_get_stats(&stats);
    printf("warmed resumes=%u system_allocations=%zu cache_hits=%zu parked_arena_bytes=0\n",
           SCALE_PROCESSES * (SCALE_ROUNDS - 1),
           stats.system_allocations - warmed.system_allocations,
           stats.cache_hits - warmed.cache_hits);
    llg_rt_cleanup();
    llg_rt_co_cache_get_stats(&stats);
    CHECK(stats.cached_bytes == 0 && stats.system_frees == stats.system_allocations);
    return 0;
}
