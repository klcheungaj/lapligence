// llg_rt.c — implementation of the llg simulation runtime (see llg_rt.h).
//
// Compiled together with vendor/libaco (aco.c + acosw.S) and the generated
// model.c by the host C compiler; never linked into the Rust binaries.

#define _GNU_SOURCE

#include "llg_rt.h"
#include "llg_container.h"
#include "aco.h"
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

// Generated budgets include function frames through the recursion guard.
#define LLG_DEFAULT_STACK_VALUES 256u

// `%t` renders into the same bounded buffers as other display conversions.
// Keep the runtime precision below that capacity so a runtime argument cannot
// request an unbounded fixed-point expansion.
#define LLG_TIMEFORMAT_MAX_PRECISION (LLG_MAX_WIDTH * 2u + 128u)

// ── Fatal boundary checks ────────────────────────────────────────────────────

static void llg_fatal_allocation(const char* what, size_t count, size_t size) {
    fprintf(stderr,
            "llg: fatal: cannot allocate %zu element(s) of %zu byte(s) for %s\n",
            count, size, what);
    abort();
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

static size_t llg_stack_values = LLG_DEFAULT_STACK_VALUES;

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

static size_t llg_coroutine_stack_size(void) {
    const size_t base = 4u << 20;
    if (llg_stack_values > (SIZE_MAX - base) / sizeof(sv4_t))
        llg_fatal_allocation("coroutine stack", llg_stack_values, sizeof(sv4_t));
    return base + llg_stack_values * sizeof(sv4_t);
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
