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

    void* over_cap = llg_co_host_chunk_alloc(LLG_CO_CHUNK_CACHE_MAX_BYTES + 1u);
    llg_co_host_chunk_free(over_cap, LLG_CO_CHUNK_CACHE_MAX_BYTES + 1u);
    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_allocations == 2 && stats.cache_hits == 1 &&
                   stats.system_frees == 1 && stats.cached_bytes == chunk_bytes &&
                   stats.cached_bytes <= LLG_CO_CHUNK_CACHE_MAX_BYTES,
               "cache cap did not return the oversized chunk to the system"))
        return 1;

    llg_rt_cleanup();
    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_allocations == 2 && stats.cache_hits == 1 &&
                   stats.system_frees == 2 && stats.cached_bytes == 0 &&
                   stats.peak_cached_bytes == chunk_bytes,
               "runtime teardown did not release cached chunks"))
        return 1;
    llg_rt_cleanup();
    llg_rt_co_cache_get_stats(&stats);
    if (!check(stats.system_frees == 2 && stats.cached_bytes == 0,
               "repeated runtime teardown changed an empty cache"))
        return 1;

    puts("coroutine chunk cache ok");
    return 0;
}
