#include "llg_rt.h"

#include <stdio.h>

static int check(int condition, const char* message) {
    if (condition) return 1;
    fprintf(stderr, "coroutine chunk cache: %s\n", message);
    return 0;
}

int main(void) {
    llg_co_arena_t arena = {0};
    llg_rt_co_cache_stats_t stats;
    const size_t chunk_bytes = sizeof(llg_co_chunk_t) + LLG_CO_ARENA_MIN_CHUNK;
    const size_t small_bytes = 128;
    const size_t target_bytes = 300;
    const size_t blocker_bytes = 400;
    const size_t large_bytes = 700;
    const size_t class_cache_bytes =
        chunk_bytes + small_bytes + target_bytes + blocker_bytes + large_bytes;

    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_allocations == 0 && stats.cache_hits == 0 &&
                   stats.system_frees == 0 && stats.cached_bytes == 0 &&
                   stats.peak_cached_bytes == 0,
               "initial counters are not zero"))
        return 1;

    void* first = llg_co_arena_grow(&arena, 64);
    llg_rt_co_cache_get_stats(&stats);
    if (!check(first != NULL && stats.system_allocations == 1 &&
                   stats.cached_bytes == 0,
               "first arena allocation did not reach the system allocator"))
        return 1;
    llg_co_arena_retire(&arena);
    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_allocations == 1 && stats.cache_hits == 0 &&
                   stats.system_frees == 0 && stats.cached_bytes == chunk_bytes &&
                   stats.peak_cached_bytes == chunk_bytes,
               "retired chunk was not cached"))
        return 1;

    void* reused = llg_co_arena_grow(&arena, 64);
    llg_rt_co_cache_get_stats(&stats);
    if (!check(reused == first && stats.system_allocations == 1 &&
                   stats.cache_hits == 1 && stats.cached_bytes == 0,
               "exact-size chunk was not reused"))
        return 1;
    llg_co_arena_retire(&arena);

    void* small = llg_co_host_chunk_alloc(small_bytes);
    void* target = llg_co_host_chunk_alloc(target_bytes);
    void* blocker = llg_co_host_chunk_alloc(blocker_bytes);
    void* large = llg_co_host_chunk_alloc(large_bytes);
    llg_co_host_chunk_free(target, target_bytes);
    llg_co_host_chunk_free(blocker, blocker_bytes);
    llg_co_host_chunk_free(small, small_bytes);
    llg_co_host_chunk_free(large, large_bytes);
    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_allocations == 5 && stats.cache_hits == 1 &&
                   stats.system_frees == 0 &&
                   stats.cached_bytes == class_cache_bytes,
               "chunks from several size classes were not cached"))
        return 1;

    reused = llg_co_host_chunk_alloc(target_bytes);
    llg_rt_co_cache_get_stats(&stats);
    if (!check(reused == target && stats.system_allocations == 5 &&
                   stats.cache_hits == 2 &&
                   stats.cached_bytes == class_cache_bytes - target_bytes,
               "exact-size chunk behind a same-class entry was not reused"))
        return 1;
    llg_co_host_chunk_free(reused, target_bytes);

    const size_t filler_bytes =
        LLG_CO_CHUNK_CACHE_MAX_BYTES - class_cache_bytes;
    void* filler = llg_co_host_chunk_alloc(filler_bytes);
    llg_co_host_chunk_free(filler, filler_bytes);
    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_allocations == 6 && stats.cache_hits == 2 &&
                   stats.system_frees == 0 &&
                   stats.cached_bytes == LLG_CO_CHUNK_CACHE_MAX_BYTES &&
                   stats.peak_cached_bytes == LLG_CO_CHUNK_CACHE_MAX_BYTES,
               "cache did not accept a chunk at the exact byte cap"))
        return 1;

    void* over_cap = llg_co_host_chunk_alloc(small_bytes + 1u);
    llg_co_host_chunk_free(over_cap, small_bytes + 1u);
    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_allocations == 7 && stats.cache_hits == 2 &&
                   stats.system_frees == 1 &&
                   stats.cached_bytes == LLG_CO_CHUNK_CACHE_MAX_BYTES,
               "free beyond the cache cap did not reach the system"))
        return 1;

    llg_rt_cleanup();
    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_allocations == 7 && stats.cache_hits == 2 &&
                   stats.system_frees == 7 && stats.cached_bytes == 0 &&
                   stats.peak_cached_bytes == LLG_CO_CHUNK_CACHE_MAX_BYTES,
               "runtime teardown did not release cached chunks"))
        return 1;
    llg_rt_cleanup();
    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_frees == 7 && stats.cached_bytes == 0,
               "repeated runtime teardown changed an empty cache"))
        return 1;

    puts("coroutine chunk cache ok");
    return 0;
}
