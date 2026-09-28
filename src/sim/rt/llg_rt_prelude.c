// llg_rt.c — implementation of the llg simulation runtime (see llg_rt.h).
//
// Compiled together with llg_co.c and the generated model by the host C
// compiler; never linked into the Rust binaries.

#define _GNU_SOURCE

#include "llg_rt.h"
#include "llg_container.h"
#ifdef LLG_WAVEFORM
#include "llg_wave.h"
#endif

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <math.h>
#include <limits.h>
#include <ctype.h>
#include <errno.h>

#if defined(__unix__) || defined(__APPLE__)
#include <sys/resource.h>
#endif

// Formatting precision is a request limit, not a storage capacity.
// Scratch allocations are sized for each value and requested conversion.
#define LLG_TIMEFORMAT_MAX_PRECISION (2u * LLG_SUPPORTED_WIDTH_LIMIT + 128u)

static void llg_warn_host_stack_limit(void) {
#if defined(__unix__) || defined(__APPLE__)
    struct rlimit limit;
    const rlim_t estimate = (rlim_t)LLG_HOST_STACK_ESTIMATE_BYTES;
    if (getrlimit(RLIMIT_STACK, &limit) == 0 &&
        limit.rlim_cur != RLIM_INFINITY && limit.rlim_cur < estimate) {
        fprintf(stderr,
                "llg: warning: host stack limit is %llu bytes; at least %llu "
                "bytes is estimated for the scheduler, one polled coroutine "
                "segment, and the 256-call recursion guard\n",
                (unsigned long long)limit.rlim_cur,
                (unsigned long long)estimate);
    }
#endif
}

// ── Fatal boundary checks ────────────────────────────────────────────────────

static _Noreturn void llg_fatal_allocation(const char* what, size_t count,
                                           size_t size) {
    fprintf(stderr,
            "llg: fatal: cannot allocate %zu element(s) of %zu byte(s) for %s\n",
            count, size, what);
    abort();
}

typedef struct llg_rt_co_cached_chunk {
    struct llg_rt_co_cached_chunk* next;
    size_t bytes;
} llg_rt_co_cached_chunk_t;

static struct {
    llg_rt_co_cached_chunk_t* heads[sizeof(size_t) * CHAR_BIT];
    llg_rt_co_cache_stats_t stats;
} llg_rt_co_chunk_cache;

static void llg_rt_co_count(size_t* counter) {
    if (*counter != SIZE_MAX) (*counter)++;
}

_Noreturn void llg_rt_co_oom(size_t bytes) {
    llg_fatal_allocation("coroutine frame", 1, bytes);
}

static llg_co_chain_t* llg_rt_current_chain(void);

static void llg_rt_co_trace_frame(void* user, const llg_co_desc_t* desc,
                                  const llg_co_frame_t* frame,
                                  unsigned depth) {
    FILE* output = (FILE*)user;
    const char* location = NULL;
    if (desc && desc->sites && frame && frame->state < desc->n_sites)
        location = desc->sites[frame->state].loc;
    fprintf(output, "  #%u %s", depth,
            desc && desc->name ? desc->name : "<unknown HDL frame>");
    if (location && location[0] != '\0') fprintf(output, " at %s", location);
    fputc('\n', output);
}

_Noreturn void llg_rt_co_bad_state(const llg_co_frame_t* co, const char* fn) {
    fprintf(stderr,
            "llg: fatal: invalid coroutine state in %s: frame=%p state=%lu\n",
            fn ? fn : "<unknown>", (const void*)co,
            co ? (unsigned long)co->state : 0ul);
    llg_co_chain_t* chain = llg_rt_current_chain();
    if (chain) {
        fputs("llg: coroutine HDL backtrace:\n", stderr);
        (void)llg_co_backtrace(chain, llg_rt_co_trace_frame, stderr);
    }
    abort();
}

static size_t llg_rt_co_size_class(size_t bytes) {
    size_t size_class = 0;
    while (bytes >>= 1) size_class++;
    return size_class;
}

void* llg_co_host_chunk_alloc(size_t bytes) {
    const size_t size_class = llg_rt_co_size_class(bytes);
    llg_rt_co_cached_chunk_t* previous = NULL;
    llg_rt_co_cached_chunk_t* chunk =
        llg_rt_co_chunk_cache.heads[size_class];
    while (chunk && chunk->bytes != bytes) {
        previous = chunk;
        chunk = chunk->next;
    }
    if (chunk) {
        if (previous)
            previous->next = chunk->next;
        else
            llg_rt_co_chunk_cache.heads[size_class] = chunk->next;
        llg_rt_co_chunk_cache.stats.cached_bytes -= bytes;
        llg_rt_co_count(&llg_rt_co_chunk_cache.stats.cache_hits);
        return chunk;
    }

    void* allocation = malloc(bytes);
    if (!allocation) llg_rt_co_oom(bytes);
    llg_rt_co_count(&llg_rt_co_chunk_cache.stats.system_allocations);
    return allocation;
}

void llg_co_host_chunk_free(void* allocation, size_t bytes) {
    const size_t cap = (size_t)LLG_CO_CHUNK_CACHE_MAX_BYTES;
    if (bytes >= sizeof(llg_rt_co_cached_chunk_t) &&
        llg_rt_co_chunk_cache.stats.cached_bytes <= cap &&
        bytes <= cap - llg_rt_co_chunk_cache.stats.cached_bytes) {
        const size_t size_class = llg_rt_co_size_class(bytes);
        llg_rt_co_cached_chunk_t* chunk = (llg_rt_co_cached_chunk_t*)allocation;
        chunk->bytes = bytes;
        chunk->next = llg_rt_co_chunk_cache.heads[size_class];
        llg_rt_co_chunk_cache.heads[size_class] = chunk;
        llg_rt_co_chunk_cache.stats.cached_bytes += bytes;
        if (llg_rt_co_chunk_cache.stats.peak_cached_bytes <
            llg_rt_co_chunk_cache.stats.cached_bytes)
            llg_rt_co_chunk_cache.stats.peak_cached_bytes =
                llg_rt_co_chunk_cache.stats.cached_bytes;
        return;
    }
    free(allocation);
    llg_rt_co_count(&llg_rt_co_chunk_cache.stats.system_frees);
}

void llg_rt_co_cache_get_stats(llg_rt_co_cache_stats_t* stats) {
    if (stats) *stats = llg_rt_co_chunk_cache.stats;
}

static void llg_rt_co_cache_release(void) {
    size_t size_class;
    for (size_class = 0;
         size_class < sizeof(llg_rt_co_chunk_cache.heads) /
                          sizeof(llg_rt_co_chunk_cache.heads[0]);
         size_class++) {
        llg_rt_co_cached_chunk_t* chunk =
            llg_rt_co_chunk_cache.heads[size_class];
        while (chunk) {
            llg_rt_co_cached_chunk_t* next = chunk->next;
            free(chunk);
            llg_rt_co_count(&llg_rt_co_chunk_cache.stats.system_frees);
            chunk = next;
        }
        llg_rt_co_chunk_cache.heads[size_class] = NULL;
    }
    llg_rt_co_chunk_cache.stats.cached_bytes = 0;
}

static void* llg_checked_malloc(size_t count, size_t size, const char* what) {
    if (size != 0 && count > SIZE_MAX / size)
        llg_fatal_allocation(what, count, size);
    size_t bytes = count * size;
    void* ptr = malloc(bytes == 0 ? 1 : bytes);
    if (!ptr) llg_fatal_allocation(what, count, size);
    return ptr;
}

static void* llg_checked_calloc(size_t count, size_t size, const char* what) {
    if (size != 0 && count > SIZE_MAX / size)
        llg_fatal_allocation(what, count, size);
    // Keep zero-sized requests non-null so callers never depend on a
    // platform-specific malloc(0)/calloc(0) result.
    if (count == 0 || size == 0) count = size = 1;
    void* ptr = calloc(count, size);
    if (!ptr) llg_fatal_allocation(what, count, size);
    return ptr;
}

static void llg_fmt_args_destroy(llg_fmt_arg_t* args, int n);
static size_t llg_format_time_integer(sv4_t value, uint64_t source_unit_fs,
                                      char* raw, size_t cap);

static const char* llg_parse_legacy_spec(const char* p, int* has_width,
                                         int* width, int* zero) {
    *has_width = 0;
    *width = 0;
    *zero = 0;
    while (*p == '-' || *p == '+' || *p == ' ' || *p == '#') p++;
    if (*p == '0') {
        *zero = 1;
        p++;
    }
    while (*p >= '0' && *p <= '9') {
        *has_width = 1;
        if (*width <= (INT_MAX - (*p - '0')) / 10)
            *width = *width * 10 + (*p - '0');
        p++;
    }
    if (*p == '.') {
        p++;
        while (*p >= '0' && *p <= '9') p++;
    }
    return p;
}

static int llg_sv4_nlimbs(uint32_t width) {
    return width == 0 ? 0 : (int)((width + 63u) / 64u);
}

static uint64_t llg_sv4_limb_mask(uint32_t width, int index) {
    int limbs = llg_sv4_nlimbs(width);
    if (index < 0 || index >= limbs) return 0;
    if (index == limbs - 1 && (width % 64) != 0)
        return LLG_MASK((uint32_t)(width % 64));
    return ~0ULL;
}

static void llg_append(char* buf, size_t cap, size_t* len, char c) {
    if (*len + 1 < cap) buf[(*len)++] = c;
}
