#define _POSIX_C_SOURCE 200809L
#include "llg_rt.h"
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static uint64_t yields_per_call;
static volatile uint64_t checksum;
#include "benchmark_calls.inc"

static void require(int condition) {
    if (!condition) abort();
}

static uint64_t nanoseconds(void) {
    struct timespec time;
    require(clock_gettime(CLOCK_MONOTONIC, &time) == 0);
    return (uint64_t)time.tv_sec * UINT64_C(1000000000) + (uint64_t)time.tv_nsec;
}

static size_t positive(const char* text) {
    char* end;
    unsigned long long value = strtoull(text, &end, 10);
    require(*text && !*end && value && value <= SIZE_MAX);
    return (size_t)value;
}

static void calls(size_t count, size_t rounds, size_t active_divisor) {
    const size_t stride = sizeof(llg_co_chain_t) + sizeof(frame_0);
    require(count <= SIZE_MAX / stride);
    unsigned char* roots = calloc(count, stride);
    require(roots != NULL);
    for (size_t index = 0; index < count; index++)
        llg_co_start((llg_co_chain_t*)(roots + index * stride), &desc_0, NULL);
    const size_t active = count / active_divisor;
    require(active > 0);
    for (size_t round = 0; round < rounds; round++) {
        for (size_t index = 0; index < active; index++) {
            llg_co_chain_t* chain = (llg_co_chain_t*)(roots + index * stride);
            require(llg_co_run(chain) == LLG_CO_PENDING);
        }
    }
    printf("operations=%zu root_bytes=%zu active=%zu checksum=%" PRIu64 " ",
           rounds * active, count * stride, active, checksum);
    for (size_t index = 0; index < count; index++) {
        llg_co_chain_t* chain = (llg_co_chain_t*)(roots + index * stride);
        llg_co_arena_release(&chain->arena);
    }
    free(roots);
}

static void arena_mix(size_t count, size_t rounds, size_t mix) {
    static const size_t sizes[][8] = {
        {32, 32, 32, 32, 32, 32, 32, 32},
        {64, 256, 1024, 4096, 16384, 1024, 256, 64},
        {16384, 65536, 16384, 65536, 16384, 65536, 16384, 65536},
    };
    require(mix < 3 && count <= SIZE_MAX / (8 * sizeof(void*)));
    llg_co_arena_t* arenas = calloc(count, sizeof(*arenas));
    void** slots = calloc(count * 8, sizeof(*slots));
    require(arenas && slots);
    size_t peak_live_bytes = 0;
    for (size_t round = 0; round < rounds; round++) {
        for (size_t index = 0; index < count; index++) {
            for (size_t slot = 0; slot < 8; slot++) {
                void* frame = llg_co_arena_push(&arenas[index], sizes[mix][slot]);
                require(frame != NULL);
                memset(frame, (int)slot, sizes[mix][slot]);
                slots[index * 8 + slot] = frame;
            }
        }
        if (round == 0) {
            for (size_t index = 0; index < count; index++) {
                for (llg_co_chunk_t* chunk = arenas[index].head; chunk; chunk = chunk->prev)
                    peak_live_bytes += sizeof(*chunk) + chunk->size;
            }
        }
        for (size_t index = 0; index < count; index++) {
            for (size_t slot = 8; slot-- > 0;) {
                unsigned char* frame = slots[index * 8 + slot];
                require(frame[0] == slot && frame[sizes[mix][slot] - 1] == slot);
                checksum += frame[0];
                llg_co_arena_pop(&arenas[index], frame);
            }
            require(arenas[index].head == NULL);
        }
    }
    printf("operations=%zu live_chunk_bytes=%zu checksum=%" PRIu64 " ",
           count * rounds * 8, peak_live_bytes, checksum);
    free(slots);
    free(arenas);
}

int main(int argc, char** argv) {
    require(argc == 6);
    const size_t count = positive(argv[2]);
    const size_t rounds = positive(argv[3]);
    yields_per_call = positive(argv[4]);
    const size_t variant = positive(argv[5]);
    const uint64_t start = nanoseconds();
    if (!strcmp(argv[1], "calls")) calls(count, rounds, variant);
    else {
        require(!strcmp(argv[1], "arena") && variant <= 3);
        arena_mix(count, rounds, variant - 1);
    }
    const uint64_t elapsed = nanoseconds() - start;
    const uint64_t expected = !strcmp(argv[1], "calls")
        ? UINT64_C(2) * rounds * (count / variant)
        : UINT64_C(28) * rounds * count;
    require(checksum == expected);
    llg_rt_co_cache_stats_t stats;
    llg_rt_co_cache_get_stats(&stats);
    require(stats.cached_bytes <= LLG_CO_CHUNK_CACHE_MAX_BYTES);
    printf("elapsed_ns=%" PRIu64 " system_allocations=%zu cache_hits=%zu "
           "system_frees=%zu cached_bytes=%zu peak_cached_bytes=%zu\n",
           elapsed, stats.system_allocations, stats.cache_hits,
           stats.system_frees, stats.cached_bytes, stats.peak_cached_bytes);
    return 0;
}
