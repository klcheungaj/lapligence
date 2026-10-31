// llg_rt.c — implementation of the llg simulation runtime (see llg_rt.h).
//
// Compiled together with llg_co.c and the generated model by the host C
// compiler; never linked into the Rust binaries.

#define _GNU_SOURCE
#define LLG_PLATFORM_CONSOLE_LOG 1

#include "llg_platform.h"
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
#include <float.h>
#include <limits.h>
#include <ctype.h>
#include <errno.h>

// Formatting precision is a request limit, not a storage capacity.
// Scratch allocations are sized for each value and requested conversion.
#define LLG_TIMEFORMAT_MAX_PRECISION (2u * LLG_SUPPORTED_WIDTH_LIMIT + 128u)

static void llg_warn_host_stack_limit(void) {
    uint64_t limit = 0;
    const uint64_t estimate = (uint64_t)LLG_HOST_STACK_ESTIMATE_BYTES;
    if (llg_host_stack_limit(&limit) && limit < estimate) {
        fprintf(stderr,
                "llg: warning: host stack limit is %llu bytes; at least %llu "
                "bytes is estimated for the scheduler, one polled coroutine "
                "segment, the 256-call depth guard, runtime helpers, and "
                "DPI/libc headroom\n",
                (unsigned long long)limit,
                (unsigned long long)estimate);
    }
}

// ── Fatal boundary checks ────────────────────────────────────────────────────

static _Noreturn void llg_fatal_allocation(const char* what, size_t count,
                                           size_t size) {
    fprintf(stderr,
            "llg: fatal: cannot allocate %zu element(s) of %zu byte(s) for %s\n",
            count, size, what);
    abort();
}

// ── Whole-message diagnostics ────────────────────────────────────────────────
//
// A diagnostic is assembled in one buffer and written with one stdio call, so
// a message from another thread (or a waveform report) can never land inside
// it. Short messages stay in the inline buffer; longer ones grow on the heap.
// An allocation failure truncates the message rather than aborting a report.

typedef struct {
    char* data;
    size_t len;
    size_t cap;
    char inline_buf[512];
} llg_report_t;

static void llg_report_init(llg_report_t* report) {
    report->data = report->inline_buf;
    report->len = 0;
    report->cap = sizeof(report->inline_buf);
}

// Make room for `extra` more bytes plus a terminator; returns how many of the
// requested bytes fit.
static size_t llg_report_reserve(llg_report_t* report, size_t extra) {
    size_t room = report->cap - report->len - 1u;
    if (extra <= room) return extra;
    size_t need = report->len + extra + 1u;
    if (need < report->len) return room;
    size_t cap = report->cap * 2u > need ? report->cap * 2u : need;
    char* grown = report->data == report->inline_buf ? (char*)malloc(cap)
                                                     : (char*)realloc(report->data, cap);
    if (!grown) return room;
    if (report->data == report->inline_buf) memcpy(grown, report->data, report->len);
    report->data = grown;
    report->cap = cap;
    return extra;
}

static void llg_report_bytes(llg_report_t* report, const char* bytes, size_t len) {
    size_t fit = llg_report_reserve(report, len);
    memcpy(report->data + report->len, bytes, fit);
    report->len += fit;
}

static void llg_report_vprintf(llg_report_t* report, const char* fmt, va_list args) {
    va_list again;
    va_copy(again, args);
    size_t room = report->cap - report->len;
    int written = vsnprintf(report->data + report->len, room, fmt, args);
    if (written < 0) {
        va_end(again);
        return;
    }
    if ((size_t)written >= room) {
        size_t fit = llg_report_reserve(report, (size_t)written);
        (void)vsnprintf(report->data + report->len, fit + 1u, fmt, again);
        written = (int)fit;
    }
    va_end(again);
    report->len += (size_t)written;
}

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 3)))
#endif
static void llg_report_printf(llg_report_t* report, const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    llg_report_vprintf(report, fmt, args);
    va_end(args);
}

// Write the message to `stream` in one call and release it.
static void llg_report_emit(llg_report_t* report, FILE* stream) {
    if (report->len) (void)fwrite(report->data, 1, report->len, stream);
    fflush(stream);
    if (report->data != report->inline_buf) free(report->data);
    llg_report_init(report);
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
static const char* llg_rt_current_name(void);

/* A root descriptor shared by every instance of one process body has no
 * name; its frame is labelled with the running process's name. */
typedef struct {
    llg_report_t* report;
    const char* root_name;
} llg_rt_co_trace_t;

static void llg_rt_co_trace_frame(void* user, const llg_co_desc_t* desc,
                                  const llg_co_frame_t* frame,
                                  unsigned depth) {
    llg_rt_co_trace_t* trace = (llg_rt_co_trace_t*)user;
    llg_report_t* report = trace->report;
    const char* location = NULL;
    if (desc && desc->sites && frame && frame->state < desc->n_sites)
        location = desc->sites[frame->state].loc;
    const char* name = desc && desc->name ? desc->name
                       : desc && trace->root_name ? trace->root_name
                                                  : "<unknown HDL frame>";
    llg_report_printf(report, "  #%u %s", depth, name);
    if (location && location[0] != '\0') llg_report_printf(report, " at %s", location);
    llg_report_bytes(report, "\n", 1);
}

_Noreturn void llg_rt_co_bad_state(const llg_co_frame_t* co, const char* fn) {
    llg_report_t report;
    llg_report_init(&report);
    llg_report_printf(&report,
                      "llg: fatal: invalid coroutine state in %s: frame=%p state=%lu\n",
                      fn ? fn : "<unknown>", (const void*)co,
                      co ? (unsigned long)co->state : 0ul);
    llg_co_chain_t* chain = llg_rt_current_chain();
    if (chain) {
        llg_report_printf(&report, "llg: coroutine HDL backtrace:\n");
        llg_rt_co_trace_t trace = {&report, llg_rt_current_name()};
        (void)llg_co_backtrace(chain, llg_rt_co_trace_frame, &trace);
    }
    llg_report_emit(&report, stderr);
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
