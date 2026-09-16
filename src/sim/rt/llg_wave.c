// llg_wave.c — bounded asynchronous VCD/FST waveform writer.
//
// One simulation thread is the producer and one file-writer thread is the
// consumer. The 1024-slot ring owns exact-width packed snapshots, not borrowed
// model limbs. Each slot still reserves room for a 1023-byte path. Snapshot
// allocations move producer -> ring -> writer using release/acquire publication
// and are destroyed after processing, including ignored/error-path events.
// Mutexes and condition variables are used for full/empty and flush waits.

#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L
#endif

#include "llg_wave.h"
#include "fstapi.h"

#include <ctype.h>
#include <errno.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <pthread.h>
#include <stdatomic.h>
#endif

#define LLG_WAVE_QUEUE_CAP 1024u
#define LLG_WAVE_PATH_CAP 1024u
#define LLG_WAVE_NO_REG UINT32_MAX
#define LLG_WAVE_HIER_SEP '\x1f'

typedef enum {
    EV_FILE,
    EV_SNAPSHOT_BEGIN,
    EV_SNAPSHOT_END,
    EV_OFF,
    EV_FLUSH,
    EV_LIMIT,
    EV_CHANGE_SV4,
    EV_CHANGE_REAL,
    EV_CLOSE
} event_kind_t;

typedef enum { SNAP_DUMPVARS, SNAP_ON, SNAP_ALL } snapshot_kind_t;
typedef enum { FORMAT_NONE, FORMAT_VCD, FORMAT_FST } wave_format_t;

typedef struct {
    event_kind_t kind;
    uint64_t now;
    uint64_t arg;
    uint32_t first_reg;
    uint8_t snapshot;
    union {
        sv4_storage_t sv4;
        double real;
        char path[LLG_WAVE_PATH_CAP];
    } payload;
} wave_event_t;

typedef struct {
    char* name;
    void* ptr;
    uint32_t width;
    uint32_t next_alias;
    uint8_t is_real;
    uint8_t selected;
} registration_t;

typedef struct {
    void* key;
    uint32_t first_reg;
} map_entry_t;

#ifdef _WIN32
typedef HANDLE wave_thread_t;
typedef CRITICAL_SECTION wave_mutex_t;
typedef CONDITION_VARIABLE wave_cond_t;
typedef DWORD wave_thread_id_t;
typedef volatile LONG64 wave_atomic_u64_t;
typedef volatile LONG wave_atomic_int_t;

static void mutex_init(wave_mutex_t* m) { InitializeCriticalSection(m); }
static void mutex_destroy(wave_mutex_t* m) { DeleteCriticalSection(m); }
static void mutex_lock(wave_mutex_t* m) { EnterCriticalSection(m); }
static void mutex_unlock(wave_mutex_t* m) { LeaveCriticalSection(m); }
static void cond_init(wave_cond_t* c) { InitializeConditionVariable(c); }
static void cond_destroy(wave_cond_t* c) { (void)c; }
static void cond_wait(wave_cond_t* c, wave_mutex_t* m) {
    SleepConditionVariableCS(c, m, INFINITE);
}
static void cond_signal(wave_cond_t* c) { WakeConditionVariable(c); }
static void cond_broadcast(wave_cond_t* c) { WakeAllConditionVariable(c); }
static wave_thread_id_t thread_self(void) { return GetCurrentThreadId(); }
static int thread_equal(wave_thread_id_t a, wave_thread_id_t b) { return a == b; }
static uint64_t atomic_u64_load(const wave_atomic_u64_t* p) {
    return (uint64_t)InterlockedCompareExchange64((LONG64 volatile*)p, 0, 0);
}
static void atomic_u64_store(wave_atomic_u64_t* p, uint64_t v) {
    InterlockedExchange64(p, (LONG64)v);
}
static int atomic_int_load(const wave_atomic_int_t* p) {
    return (int)InterlockedCompareExchange((LONG volatile*)p, 0, 0);
}
static void atomic_int_store(wave_atomic_int_t* p, int v) {
    InterlockedExchange(p, (LONG)v);
}
static int atomic_int_cas_zero(wave_atomic_int_t* p) {
    return InterlockedCompareExchange(p, 1, 0) == 0;
}
#define WAVE_THREAD_RETURN DWORD WINAPI
#define WAVE_THREAD_RESULT 0
#else
typedef pthread_t wave_thread_t;
typedef pthread_mutex_t wave_mutex_t;
typedef pthread_cond_t wave_cond_t;
typedef pthread_t wave_thread_id_t;
typedef _Atomic uint64_t wave_atomic_u64_t;
typedef _Atomic int wave_atomic_int_t;

static void mutex_init(wave_mutex_t* m) { (void)pthread_mutex_init(m, NULL); }
static void mutex_destroy(wave_mutex_t* m) { (void)pthread_mutex_destroy(m); }
static void mutex_lock(wave_mutex_t* m) { (void)pthread_mutex_lock(m); }
static void mutex_unlock(wave_mutex_t* m) { (void)pthread_mutex_unlock(m); }
static void cond_init(wave_cond_t* c) { (void)pthread_cond_init(c, NULL); }
static void cond_destroy(wave_cond_t* c) { (void)pthread_cond_destroy(c); }
static void cond_wait(wave_cond_t* c, wave_mutex_t* m) {
    (void)pthread_cond_wait(c, m);
}
static void cond_signal(wave_cond_t* c) { (void)pthread_cond_signal(c); }
static void cond_broadcast(wave_cond_t* c) { (void)pthread_cond_broadcast(c); }
static wave_thread_id_t thread_self(void) { return pthread_self(); }
static int thread_equal(wave_thread_id_t a, wave_thread_id_t b) {
    return pthread_equal(a, b);
}
static uint64_t atomic_u64_load(const wave_atomic_u64_t* p) {
    return atomic_load_explicit(p, memory_order_acquire);
}
static void atomic_u64_store(wave_atomic_u64_t* p, uint64_t v) {
    atomic_store_explicit(p, v, memory_order_release);
}
static int atomic_int_load(const wave_atomic_int_t* p) {
    return atomic_load_explicit(p, memory_order_acquire);
}
static void atomic_int_store(wave_atomic_int_t* p, int v) {
    atomic_store_explicit(p, v, memory_order_release);
}
static int atomic_int_cas_zero(wave_atomic_int_t* p) {
    int expected = 0;
    return atomic_compare_exchange_strong_explicit(
        p, &expected, 1, memory_order_acq_rel, memory_order_acquire);
}
#define WAVE_THREAD_RETURN void*
#define WAVE_THREAD_RESULT NULL
#endif

typedef struct {
    FILE* file;
    void* fst;
    wave_format_t format;
    char* path;
    fstHandle* fst_handles;
    char* packed_text;
    size_t packed_text_cap;
    uint64_t last_time;
    uint64_t bytes_written;
    uint64_t byte_limit;
    int have_time;
    int header_written;
    int active;
    int snapshot_open;
    int limit_reached;
} writer_t;

typedef struct {
    registration_t* regs;
    uint32_t reg_count;
    uint32_t reg_cap;
    map_entry_t* map;
    uint32_t map_cap;
    uint64_t precision_fs;
    wave_event_t queue[LLG_WAVE_QUEUE_CAP];
    wave_atomic_u64_t head;
    wave_atomic_u64_t tail;
    wave_atomic_u64_t ack;
    wave_atomic_int_t error;
    wave_atomic_int_t worker_alive;
    wave_atomic_int_t producer_waiting;
    wave_atomic_int_t consumer_waiting;
    wave_mutex_t mutex;
    wave_cond_t not_empty;
    wave_cond_t not_full;
    wave_cond_t ack_changed;
    wave_thread_t worker;
    wave_thread_id_t producer;
    int initialized;
    int worker_started;
    int producer_dumping;
} wave_state_t;

static wave_state_t g_wave;

static char* wave_strdup(const char* s) {
    size_t n = strlen(s) + 1u;
    char* copy = (char*)malloc(n);
    if (copy) memcpy(copy, s, n);
    return copy;
}

static void wave_error(const char* fmt, ...) {
    if (!atomic_int_cas_zero(&g_wave.error)) return;
    va_list ap;
    va_start(ap, fmt);
    fputs("llg: waveform: ", stderr);
    vfprintf(stderr, fmt, ap);
    fputc('\n', stderr);
    va_end(ap);
}

static int require_producer(const char* operation) {
    if (!g_wave.initialized) {
        fprintf(stderr, "llg: waveform: %s called before llg_wave_model_init\n",
                operation);
        return 0;
    }
    if (!thread_equal(g_wave.producer, thread_self())) {
        wave_error("%s called from a second producer thread", operation);
        return 0;
    }
    return 1;
}

static uint32_t hash_ptr(const void* ptr) {
    uintptr_t x = (uintptr_t)ptr;
    x ^= x >> 33;
    x *= (uintptr_t)0xff51afd7ed558ccdULL;
    x ^= x >> 33;
    return (uint32_t)x;
}

static int freeze_registrations(void) {
    uint32_t cap = 8u;
    while (cap < g_wave.reg_count * 2u) cap <<= 1u;
    g_wave.map = (map_entry_t*)calloc(cap, sizeof(map_entry_t));
    if (!g_wave.map) {
        wave_error("out of memory while freezing signal registrations");
        return 0;
    }
    g_wave.map_cap = cap;
    for (uint32_t i = 0; i < g_wave.reg_count; i++) {
        registration_t* reg = &g_wave.regs[i];
        uint32_t slot = hash_ptr(reg->ptr) & (cap - 1u);
        while (g_wave.map[slot].key && g_wave.map[slot].key != reg->ptr)
            slot = (slot + 1u) & (cap - 1u);
        if (!g_wave.map[slot].key) {
            g_wave.map[slot].key = reg->ptr;
            g_wave.map[slot].first_reg = i;
        } else {
            uint32_t alias = g_wave.map[slot].first_reg;
            if (g_wave.regs[alias].is_real != reg->is_real ||
                g_wave.regs[alias].width != reg->width) {
                wave_error("aliases of `%s` have incompatible waveform types",
                           reg->name);
                return 0;
            }
            while (g_wave.regs[alias].next_alias != LLG_WAVE_NO_REG)
                alias = g_wave.regs[alias].next_alias;
            g_wave.regs[alias].next_alias = i;
        }
    }
    return 1;
}

static uint32_t lookup_registration(const void* ptr) {
    if (!g_wave.map_cap || !ptr) return LLG_WAVE_NO_REG;
    uint32_t slot = hash_ptr(ptr) & (g_wave.map_cap - 1u);
    while (g_wave.map[slot].key) {
        if (g_wave.map[slot].key == ptr) return g_wave.map[slot].first_reg;
        slot = (slot + 1u) & (g_wave.map_cap - 1u);
    }
    return LLG_WAVE_NO_REG;
}

static void event_destroy(wave_event_t* event) {
    if (event->kind == EV_CHANGE_SV4)
        sv4_storage_destroy(&event->payload.sv4);
    *event = (wave_event_t){0};
}

// Destination is an empty ring slot or a new local. Clearing source is part of
// the transfer; it must happen before publishing head/tail to the other thread.
static void event_move(wave_event_t* destination, wave_event_t* source) {
    *destination = *source;
    *source = (wave_event_t){0};
}

// Consumes the event, including its snapshot allocation.
static void queue_push(wave_event_t* event) {
    uint64_t head = atomic_u64_load(&g_wave.head);
    uint64_t tail = atomic_u64_load(&g_wave.tail);
    if (head - tail >= LLG_WAVE_QUEUE_CAP) {
        mutex_lock(&g_wave.mutex);
        atomic_int_store(&g_wave.producer_waiting, 1);
        while (head - atomic_u64_load(&g_wave.tail) >= LLG_WAVE_QUEUE_CAP) {
            cond_wait(&g_wave.not_full, &g_wave.mutex);
            head = atomic_u64_load(&g_wave.head);
        }
        atomic_int_store(&g_wave.producer_waiting, 0);
        mutex_unlock(&g_wave.mutex);
    }

    event_move(&g_wave.queue[head % LLG_WAVE_QUEUE_CAP], event);
    atomic_u64_store(&g_wave.head, head + 1u);
    if (atomic_int_load(&g_wave.consumer_waiting)) {
        mutex_lock(&g_wave.mutex);
        cond_signal(&g_wave.not_empty);
        mutex_unlock(&g_wave.mutex);
    }
}

static wave_event_t queue_pop(void) {
    uint64_t tail = atomic_u64_load(&g_wave.tail);
    uint64_t head = atomic_u64_load(&g_wave.head);
    if (tail == head) {
        mutex_lock(&g_wave.mutex);
        atomic_int_store(&g_wave.consumer_waiting, 1);
        while (tail == atomic_u64_load(&g_wave.head)) {
            cond_wait(&g_wave.not_empty, &g_wave.mutex);
            tail = atomic_u64_load(&g_wave.tail);
        }
        atomic_int_store(&g_wave.consumer_waiting, 0);
        mutex_unlock(&g_wave.mutex);
    }

    wave_event_t event = {0};
    event_move(&event, &g_wave.queue[tail % LLG_WAVE_QUEUE_CAP]);
    atomic_u64_store(&g_wave.tail, tail + 1u);
    if (atomic_int_load(&g_wave.producer_waiting)) {
        mutex_lock(&g_wave.mutex);
        cond_signal(&g_wave.not_full);
        mutex_unlock(&g_wave.mutex);
    }
    return event;
}

static const char* path_extension(const char* path) {
    const char* dot = strrchr(path, '.');
    const char* slash = strrchr(path, '/');
    const char* backslash = strrchr(path, '\\');
    if (!dot || (slash && dot < slash) || (backslash && dot < backslash))
        return "";
    return dot;
}

static int extension_is(const char* ext, const char* wanted) {
    while (*ext && *wanted) {
        if (tolower((unsigned char)*ext++) != tolower((unsigned char)*wanted++))
            return 0;
    }
    return *ext == 0 && *wanted == 0;
}

// Encode one source identifier component without loss. `$` is reserved as the
// escape prefix and therefore encoded itself; this makes the transformation
// injective (`a-b`, `a_b`, and `a$b` can never collapse to one trace name).
// A leading-digit marker is likewise unambiguous because a source `$` never
// passes through literally.  Hierarchy boundaries are handled separately.
static char* serialize_identifier(const char* src, size_t len) {
    static const char hex[] = "0123456789ABCDEF";
    if (len > (SIZE_MAX - 8u) / 3u) {
        wave_error("waveform identifier is too long");
        return NULL;
    }
    char* dst = (char*)malloc(len * 3u + 8u);
    if (!dst) {
        wave_error("out of memory while serializing waveform hierarchy");
        return NULL;
    }
    size_t out = 0;
    if (!len) {
        memcpy(dst, "$01", 3u);
        out = 3u;
    } else if (src[0] >= '0' && src[0] <= '9') {
        memcpy(dst, "$00", 3u);
        out = 3u;
    }
    for (size_t i = 0; i < len; i++) {
        unsigned char c = (unsigned char)src[i];
        int ascii_alnum = (c >= 'a' && c <= 'z') ||
                          (c >= 'A' && c <= 'Z') ||
                          (c >= '0' && c <= '9');
        if (ascii_alnum || c == '_') {
            dst[out++] = (char)c;
        } else {
            dst[out++] = '$';
            dst[out++] = hex[c >> 4u];
            dst[out++] = hex[c & 0x0fu];
        }
    }
    dst[out] = 0;
    return dst;
}

static void compact_id(uint32_t value, char out[8]) {
    size_t n = 0;
    do {
        out[n++] = (char)('!' + (value % 94u));
        value /= 94u;
    } while (value && n < 7u);
    out[n] = 0;
}

static void precision_string(uint64_t fs, char out[32]) {
    static const struct { uint64_t fs; const char* name; } units[] = {
        {1000000000000000ULL, "s"}, {1000000000000ULL, "ms"},
        {1000000000ULL, "us"}, {1000000ULL, "ns"},
        {1000ULL, "ps"}, {1ULL, "fs"}
    };
    if (!fs) fs = 1;
    for (size_t i = 0; i < sizeof(units) / sizeof(units[0]); i++) {
        if (fs % units[i].fs) continue;
        uint64_t scale = fs / units[i].fs;
        if (scale == 1u || scale == 10u || scale == 100u) {
            (void)snprintf(out, 32u, "%llu%s", (unsigned long long)scale,
                           units[i].name);
            return;
        }
    }
    (void)snprintf(out, 32u, "%llufs", (unsigned long long)fs);
}

static int writer_bytes(writer_t* w, const char* bytes, size_t len) {
    if (!w->file || w->limit_reached) return 0;
    if (w->byte_limit &&
        (w->bytes_written > w->byte_limit ||
         len > w->byte_limit - w->bytes_written)) {
        w->limit_reached = 1;
        return 0;
    }
    if (fwrite(bytes, 1u, len, w->file) != len) {
        wave_error("write failed for `%s`: %s", w->path, strerror(errno));
        return 0;
    }
    w->bytes_written += len;
    return 1;
}

static int writer_printf(writer_t* w, const char* fmt, ...) {
    char buf[4096];
    va_list ap;
    va_start(ap, fmt);
    int n = vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    if (n < 0 || (size_t)n >= sizeof(buf)) {
        wave_error("internal VCD record exceeded its fixed buffer");
        return 0;
    }
    return writer_bytes(w, buf, (size_t)n);
}

static uint32_t canonical_index(uint32_t reg_index) {
    return lookup_registration(g_wave.regs[reg_index].ptr);
}

static uint32_t selected_canonical(uint32_t reg_index) {
    uint32_t first = canonical_index(reg_index);
    if (first == LLG_WAVE_NO_REG) return first;
    for (uint32_t i = first; i != LLG_WAVE_NO_REG;
         i = g_wave.regs[i].next_alias) {
        if (g_wave.regs[i].selected) return i;
    }
    return LLG_WAVE_NO_REG;
}

static uint32_t separator_count(const char* name) {
    uint32_t count = 0;
    if (!name) return 0;
    for (const char* p = name; *p; p++)
        if (*p == LLG_WAVE_HIER_SEP) count++;
    return count;
}

static int valid_hierarchy_name(const char* name) {
    if (!name || !*name || name[0] == LLG_WAVE_HIER_SEP) return 0;
    size_t length = strlen(name);
    if (name[length - 1u] == LLG_WAVE_HIER_SEP) return 0;
    for (size_t i = 1u; i < length; i++) {
        if (name[i] == LLG_WAVE_HIER_SEP &&
            name[i - 1u] == LLG_WAVE_HIER_SEP)
            return 0;
    }
    return 1;
}

static int registration_matches(const registration_t* reg, const char* selection,
                                uint32_t depth) {
    if (!reg || !valid_hierarchy_name(reg->name) ||
        !valid_hierarchy_name(selection))
        return 0;
    size_t registration_len = strlen(reg->name);
    size_t selection_len = strlen(selection);
    if (selection_len > registration_len ||
        memcmp(reg->name, selection, selection_len) != 0)
        return 0;
    if (registration_len == selection_len) return 1;
    if (reg->name[selection_len] == '[') return 1;
    if (reg->name[selection_len] != LLG_WAVE_HIER_SEP) return 0;
    if (depth == 0) return 1;
    uint32_t reg_depth = separator_count(reg->name);
    uint32_t selection_depth = separator_count(selection);
    return reg_depth >= selection_depth &&
           reg_depth - selection_depth <= depth;
}

static void select_registrations(uint32_t depth, const char* const* names,
                                 uint32_t name_count) {
    for (uint32_t i = 0; i < g_wave.reg_count; i++) {
        registration_t* reg = &g_wave.regs[i];
        reg->selected = name_count == 0 && depth == 0;
        if (reg->selected) continue;
        for (uint32_t name_index = 0; name_index < name_count; name_index++) {
            if (names[name_index] &&
                registration_matches(reg, names[name_index], depth)) {
                reg->selected = 1;
                break;
            }
        }
    }
}

static uint32_t scope_count(const char* name) {
    uint32_t count = 0;
    for (const char* p = name; *p; p++)
        if (*p == LLG_WAVE_HIER_SEP) count++;
    return count;
}

static void scope_segment(const char* name, uint32_t wanted, const char** start,
                          size_t* len) {
    const char* part = name;
    for (uint32_t i = 0; i < wanted; i++) {
        const char* separator = strchr(part, LLG_WAVE_HIER_SEP);
        if (!separator) {
            *start = part;
            *len = strlen(part);
            return;
        }
        part = separator + 1;
    }
    const char* dot = strchr(part, LLG_WAVE_HIER_SEP);
    *start = part;
    *len = dot ? (size_t)(dot - part) : strlen(part);
}

static uint32_t common_scope_count(const char* a, const char* b) {
    if (!a || !b) return 0;
    uint32_t limit = scope_count(a);
    uint32_t b_count = scope_count(b);
    if (b_count < limit) limit = b_count;
    uint32_t common = 0;
    while (common < limit) {
        const char* a_part;
        const char* b_part;
        size_t a_len;
        size_t b_len;
        scope_segment(a, common, &a_part, &a_len);
        scope_segment(b, common, &b_part, &b_len);
        if (a_len != b_len || memcmp(a_part, b_part, a_len) != 0) break;
        common++;
    }
    return common;
}

static int registration_name_compare(const void* lhs, const void* rhs) {
    uint32_t a = *(const uint32_t*)lhs;
    uint32_t b = *(const uint32_t*)rhs;
    int order = strcmp(g_wave.regs[a].name, g_wave.regs[b].name);
    if (order) return order;
    return a < b ? -1 : a > b;
}

static uint32_t* sorted_registrations(void) {
    uint32_t* order = (uint32_t*)malloc(
        (size_t)(g_wave.reg_count ? g_wave.reg_count : 1u) * sizeof(uint32_t));
    if (!order) {
        wave_error("out of memory while ordering waveform hierarchy");
        return NULL;
    }
    for (uint32_t i = 0; i < g_wave.reg_count; i++) order[i] = i;
    qsort(order, g_wave.reg_count, sizeof(uint32_t), registration_name_compare);
    return order;
}

static void scope_transition(writer_t* w, const char* previous,
                             const char* current) {
    uint32_t previous_count = previous ? scope_count(previous) : 0;
    uint32_t current_count = current ? scope_count(current) : 0;
    uint32_t common = common_scope_count(previous, current);
    for (uint32_t i = common; i < previous_count; i++) {
        if (w->format == FORMAT_VCD) writer_printf(w, "$upscope $end\n");
        else fstWriterSetUpscope(w->fst);
    }
    for (uint32_t i = common; i < current_count; i++) {
        const char* part;
        size_t len;
        scope_segment(current, i, &part, &len);
        char* serialized = serialize_identifier(part, len);
        if (!serialized) return;
        if (w->format == FORMAT_VCD) {
            writer_bytes(w, "$scope module ", 14u);
            writer_bytes(w, serialized, strlen(serialized));
            writer_bytes(w, " $end\n", 6u);
        } else {
            fstWriterSetScope(w->fst, FST_ST_VCD_MODULE, serialized, NULL);
        }
        free(serialized);
    }
}

static char* leaf_name(const char* name) {
    const char* leaf = strrchr(name, LLG_WAVE_HIER_SEP);
    leaf = leaf ? leaf + 1 : name;
    return serialize_identifier(leaf, strlen(leaf));
}

static void vcd_var(writer_t* w, uint32_t reg_index) {
    const registration_t* reg = &g_wave.regs[reg_index];
    char* serialized = leaf_name(reg->name);
    if (!serialized) return;
    char id[8];
    compact_id(selected_canonical(reg_index), id);
    if (reg->is_real) {
        writer_printf(w, "$var real 64 %s ", id);
    } else {
        writer_printf(w, "$var wire %u %s ", reg->width, id);
    }
    writer_bytes(w, serialized, strlen(serialized));
    writer_bytes(w, " $end\n", 6u);
    free(serialized);
}

static void vcd_header(writer_t* w) {
    char timescale[32];
    precision_string(g_wave.precision_fs, timescale);
    writer_printf(w, "$date\n  reproducible build\n$end\n");
    writer_printf(w, "$version\n  Lapligence asynchronous waveform writer\n$end\n");
    writer_printf(w, "$timescale %s $end\n", timescale);
    uint32_t* order = sorted_registrations();
    if (!order) return;
    const char* previous = NULL;
    for (uint32_t pos = 0; pos < g_wave.reg_count; pos++) {
        uint32_t i = order[pos];
        if (!g_wave.regs[i].selected) continue;
        scope_transition(w, previous, g_wave.regs[i].name);
        vcd_var(w, i);
        previous = g_wave.regs[i].name;
    }
    scope_transition(w, previous, NULL);
    free(order);
    writer_printf(w, "$enddefinitions $end\n");
}

static void fst_header(writer_t* w) {
    char timescale[32];
    precision_string(g_wave.precision_fs, timescale);
    fstWriterSetVersion(w->fst, "Lapligence asynchronous waveform writer");
    fstWriterSetDate(w->fst, "reproducible build");
    fstWriterSetTimescaleFromString(w->fst, timescale);
    fstWriterSetParallelMode(w->fst, 0);
    fstWriterSetPackType(w->fst, FST_WR_PT_ZLIB);
    w->fst_handles = (fstHandle*)calloc(g_wave.reg_count ? g_wave.reg_count : 1u,
                                        sizeof(fstHandle));
    if (!w->fst_handles) {
        wave_error("out of memory while creating FST handles");
        return;
    }
    uint32_t* order = sorted_registrations();
    if (!order) return;
    const char* previous = NULL;
    for (uint32_t pos = 0; pos < g_wave.reg_count; pos++) {
        uint32_t i = order[pos];
        const registration_t* reg = &g_wave.regs[i];
        if (!reg->selected) continue;
        scope_transition(w, previous, reg->name);
        char* serialized = leaf_name(reg->name);
        if (!serialized) {
            free(order);
            return;
        }
        uint32_t canonical = selected_canonical(i);
        fstHandle alias = w->fst_handles[canonical];
        enum fstVarType type = reg->is_real ? FST_VT_VCD_REAL : FST_VT_VCD_WIRE;
        fstHandle handle = fstWriterCreateVar(
            w->fst, type, FST_VD_IMPLICIT, reg->is_real ? 64u : reg->width,
            serialized, alias);
        free(serialized);
        if (!handle) {
            wave_error("failed to create FST variable `%s`", reg->name);
            free(order);
            return;
        }
        if (!alias) w->fst_handles[canonical] = handle;
        w->fst_handles[i] = handle;
        previous = reg->name;
    }
    scope_transition(w, previous, NULL);
    free(order);
}

static void writer_close_file(writer_t* w) {
    if (w->fst) {
        fstWriterClose(w->fst);
        w->fst = NULL;
    }
    if (w->file) {
        if (fclose(w->file) != 0)
            wave_error("close failed for `%s`: %s", w->path, strerror(errno));
        w->file = NULL;
    }
    free(w->fst_handles);
    w->fst_handles = NULL;
    free(w->path);
    w->path = NULL;
    free(w->packed_text);
    w->packed_text = NULL;
    w->packed_text_cap = 0;
    w->format = FORMAT_NONE;
}

static int writer_open(writer_t* w, const char* path) {
    if (w->format != FORMAT_NONE) {
        wave_error("$dumpfile cannot change `%s` to `%s` after writing started",
                   w->path, path);
        return 0;
    }
    const char* ext = path_extension(path);
    if (extension_is(ext, ".vcd")) w->format = FORMAT_VCD;
    else if (extension_is(ext, ".fst")) w->format = FORMAT_FST;
    else {
        wave_error("unsupported waveform extension in `%s` (expected .vcd or .fst)",
                   path);
        return 0;
    }
    w->path = wave_strdup(path);
    if (!w->path) {
        wave_error("out of memory while opening `%s`", path);
        return 0;
    }
    if (w->format == FORMAT_VCD) {
        w->file = fopen(path, "wb");
        if (!w->file) {
            wave_error("cannot open `%s`: %s", path, strerror(errno));
            w->format = FORMAT_NONE;
            return 0;
        }
    } else {
        w->fst = fstWriterCreate(path, 1);
        if (!w->fst) {
            wave_error("cannot create FST file `%s`", path);
            w->format = FORMAT_NONE;
            return 0;
        }
    }
    return !atomic_int_load(&g_wave.error);
}

static int writer_write_header(writer_t* w) {
    if (w->header_written) return 1;
    if (w->format == FORMAT_VCD) vcd_header(w);
    else if (w->format == FORMAT_FST) fst_header(w);
    w->header_written = 1;
    return !atomic_int_load(&g_wave.error);
}

static int ensure_writer_open(writer_t* w) {
    return w->format != FORMAT_NONE || writer_open(w, "dump.vcd");
}

static void acknowledge(uint64_t sequence) {
    atomic_u64_store(&g_wave.ack, sequence);
    mutex_lock(&g_wave.mutex);
    cond_broadcast(&g_wave.ack_changed);
    mutex_unlock(&g_wave.mutex);
}

static void writer_time(writer_t* w, uint64_t now) {
    if (w->have_time && w->last_time == now) return;
    if (w->format == FORMAT_VCD) writer_printf(w, "#%llu\n", (unsigned long long)now);
    else if (w->format == FORMAT_FST) fstWriterEmitTimeChange(w->fst, now);
    w->last_time = now;
    w->have_time = 1;
}

static void sv4_text(const sv4_storage_t* value, uint32_t width, char* out) {
    for (uint32_t pos = 0; pos < width; pos++) {
        uint32_t bit = width - 1u - pos;
        // Registrations may request a wider view than the captured value. The
        // legacy representation exposed zero padding there; never read past
        // an exact-width allocation to provide that same zero extension.
        if (bit >= value->width) {
            out[pos] = '0';
            continue;
        }
        uint64_t mask = UINT64_C(1) << (bit & 63u);
        uint32_t limb = bit >> 6u;
        if (value->x[limb] & mask) out[pos] = 'x';
        else if (value->z[limb] & mask) out[pos] = 'z';
        else out[pos] = (value->bits[limb] & mask) ? '1' : '0';
    }
    out[width] = 0;
}

static void writer_sv4_aliases(writer_t* w, uint32_t first,
                               const sv4_storage_t* value) {
    for (uint32_t i = first; i != LLG_WAVE_NO_REG; i = g_wave.regs[i].next_alias) {
        const registration_t* reg = &g_wave.regs[i];
        if (!reg->selected) continue;
        if (reg->width >= LLG_SUPPORTED_WIDTH_LIMIT) {
            wave_error("packed waveform width exceeds supported limit");
            return;
        }
        size_t required = (size_t)reg->width + 1u;
        if (required > w->packed_text_cap) {
            char* text = (char*)realloc(w->packed_text, required);
            if (!text) {
                wave_error("out of memory while formatting packed waveform value");
                return;
            }
            w->packed_text = text;
            w->packed_text_cap = required;
        }
        char* bits = w->packed_text;
        sv4_text(value, reg->width, bits);
        if (w->format == FORMAT_VCD) {
            char id[8];
            compact_id(i, id);
            if (reg->width == 1u) writer_printf(w, "%c%s\n", bits[0], id);
            else writer_printf(w, "b%s %s\n", bits, id);
            break; // aliases share the same VCD identifier
        }
        fstWriterEmitValueChange(w->fst, w->fst_handles[i], bits);
        break; // libfst aliases share the canonical handle
    }
}

static void writer_real_aliases(writer_t* w, uint32_t first, double value) {
    while (first != LLG_WAVE_NO_REG && !g_wave.regs[first].selected)
        first = g_wave.regs[first].next_alias;
    if (first == LLG_WAVE_NO_REG) return;
    if (w->format == FORMAT_VCD) {
        char id[8];
        compact_id(first, id);
        writer_printf(w, "r%.17g %s\n", value, id);
    } else {
        fstWriterEmitValueChange(w->fst, w->fst_handles[first], &value);
    }
}

static void vcd_off_values(writer_t* w) {
    for (uint32_t i = 0; i < g_wave.reg_count; i++) {
        if (!g_wave.regs[i].selected || selected_canonical(i) != i) continue;
        char id[8];
        compact_id(i, id);
        if (g_wave.regs[i].is_real) writer_printf(w, "rNaN %s\n", id);
        else if (g_wave.regs[i].width == 1u) writer_printf(w, "x%s\n", id);
        else writer_printf(w, "bx %s\n", id);
    }
}

static void process_event(writer_t* w, const wave_event_t* e) {
    if (e->kind == EV_FILE) {
        if (!atomic_int_load(&g_wave.error)) (void)writer_open(w, e->payload.path);
        return;
    }
    if (atomic_int_load(&g_wave.error)) {
        if (e->kind == EV_FLUSH) acknowledge(e->arg);
        return;
    }
    if (!ensure_writer_open(w)) {
        if (e->kind == EV_FLUSH) acknowledge(e->arg);
        return;
    }
    switch (e->kind) {
        case EV_SNAPSHOT_BEGIN: {
            if (!writer_write_header(w)) break;
            writer_time(w, e->now);
            snapshot_kind_t kind = (snapshot_kind_t)e->arg;
            if (kind == SNAP_DUMPVARS || kind == SNAP_ON) w->active = 1;
            if (w->format == FORMAT_VCD) {
                const char* directive = kind == SNAP_DUMPVARS ? "$dumpvars\n" :
                                        kind == SNAP_ON ? "$dumpon\n" : "$dumpall\n";
                writer_bytes(w, directive, strlen(directive));
                w->snapshot_open = 1;
            } else if (kind == SNAP_DUMPVARS || kind == SNAP_ON) {
                fstWriterEmitDumpActive(w->fst, 1);
            }
            break;
        }
        case EV_SNAPSHOT_END:
            if (w->format == FORMAT_VCD && w->snapshot_open) {
                writer_bytes(w, "$end\n", 5u);
                w->snapshot_open = 0;
            }
            break;
        case EV_OFF:
            if (!writer_write_header(w)) break;
            writer_time(w, e->now);
            w->active = 0;
            if (w->format == FORMAT_VCD) {
                writer_bytes(w, "$dumpoff\n", 9u);
                vcd_off_values(w);
                writer_bytes(w, "$end\n", 5u);
            } else {
                fstWriterEmitDumpActive(w->fst, 0);
            }
            break;
        case EV_CHANGE_SV4:
            if (w->active || e->snapshot) {
                if (!writer_write_header(w)) break;
                writer_time(w, e->now);
                writer_sv4_aliases(w, e->first_reg, &e->payload.sv4);
            }
            break;
        case EV_CHANGE_REAL:
            if (w->active || e->snapshot) {
                if (!writer_write_header(w)) break;
                writer_time(w, e->now);
                writer_real_aliases(w, e->first_reg, e->payload.real);
            }
            break;
        case EV_LIMIT:
            w->byte_limit = e->arg;
            if (w->format == FORMAT_FST) fstWriterSetDumpSizeLimit(w->fst, e->arg);
            break;
        case EV_FLUSH:
            if (!writer_write_header(w)) {
                acknowledge(e->arg);
                break;
            }
            writer_time(w, e->now);
            if (w->format == FORMAT_VCD) {
                if (fflush(w->file) != 0)
                    wave_error("flush failed for `%s`: %s", w->path, strerror(errno));
            } else {
                fstWriterFlushContext(w->fst);
                if (fstWriterGetFseekFailed(w->fst))
                    wave_error("FST seek/write failed for `%s`", w->path);
            }
            acknowledge(e->arg);
            break;
        case EV_CLOSE:
        case EV_FILE:
            break;
    }
}

static WAVE_THREAD_RETURN writer_thread(void* unused) {
    (void)unused;
    writer_t writer;
    memset(&writer, 0, sizeof(writer));
    for (;;) {
        wave_event_t event = queue_pop();
        if (event.kind == EV_CLOSE) {
            if (writer.format != FORMAT_NONE) {
                (void)writer_write_header(&writer);
                writer_time(&writer, event.now);
                if (writer.format == FORMAT_FST &&
                    fstWriterGetFseekFailed(writer.fst))
                    wave_error("FST seek/write failed for `%s`", writer.path);
            }
            writer_close_file(&writer);
            event_destroy(&event);
            break;
        }
        process_event(&writer, &event);
        // process_event can return early after an output error or ignore a
        // disabled/unselected value; ownership ends here on every such path.
        event_destroy(&event);
    }
    atomic_int_store(&g_wave.worker_alive, 0);
    mutex_lock(&g_wave.mutex);
    cond_broadcast(&g_wave.ack_changed);
    cond_broadcast(&g_wave.not_full);
    mutex_unlock(&g_wave.mutex);
    return WAVE_THREAD_RESULT;
}

static int start_worker(void) {
    if (g_wave.worker_started) return 1;
    if (atomic_int_load(&g_wave.error)) return 0;
    if (!freeze_registrations()) return 0;
    atomic_int_store(&g_wave.worker_alive, 1);
#ifdef _WIN32
    g_wave.worker = CreateThread(NULL, 0, writer_thread, NULL, 0, NULL);
    if (!g_wave.worker) {
        atomic_int_store(&g_wave.worker_alive, 0);
        wave_error("cannot start writer thread (Win32 error %lu)",
                   (unsigned long)GetLastError());
        return 0;
    }
#else
    int rc = pthread_create(&g_wave.worker, NULL, writer_thread, NULL);
    if (rc != 0) {
        atomic_int_store(&g_wave.worker_alive, 0);
        wave_error("cannot start writer thread: %s", strerror(rc));
        return 0;
    }
#endif
    g_wave.worker_started = 1;
    return 1;
}

static void join_worker(void) {
#ifdef _WIN32
    (void)WaitForSingleObject(g_wave.worker, INFINITE);
    CloseHandle(g_wave.worker);
#else
    (void)pthread_join(g_wave.worker, NULL);
#endif
}

static void enqueue_simple(event_kind_t kind, uint64_t now, uint64_t arg) {
    wave_event_t event = {0};
    event.kind = kind;
    event.now = now;
    event.arg = arg;
    queue_push(&event);
}

// Bridge from the legacy fixed-array value. Remove this legacy source-capacity
// guard when sv4_t itself is converted; the storage constructor has no such cap.
// The destination event is newly initialized and owns no previous snapshot.
static int capture_sv4(wave_event_t* event, const sv4_t* value) {
    if (value->width > LLG_MAX_WIDTH) {
        wave_error("packed source width exceeds legacy value capacity");
        return 0;
    }
    event->payload.sv4 = sv4_storage_from_limbs(
        value->bits, value->x, value->z, value->width, value->is_signed);
    return 1;
}

static void enqueue_snapshot(uint64_t now, snapshot_kind_t kind) {
    if (!start_worker()) return;
    enqueue_simple(EV_SNAPSHOT_BEGIN, now, (uint64_t)kind);
    for (uint32_t i = 0; i < g_wave.reg_count; i++) {
        registration_t* reg = &g_wave.regs[i];
        if (lookup_registration(reg->ptr) != i) continue;
        uint32_t selected = selected_canonical(i);
        if (selected == LLG_WAVE_NO_REG) continue;
        wave_event_t event = {0};
        event.kind = reg->is_real ? EV_CHANGE_REAL : EV_CHANGE_SV4;
        event.now = now;
        event.first_reg = selected;
        event.snapshot = 1;
        if (reg->is_real) event.payload.real = *(double*)reg->ptr;
        else if (!capture_sv4(&event, (const sv4_t*)reg->ptr)) continue;
        queue_push(&event);
    }
    enqueue_simple(EV_SNAPSHOT_END, now, 0);
}

static void free_state(void) {
    // The worker has joined (or never started). Normally every slot is already
    // empty after queue_pop; retain this cleanup for any unconsumed owners.
    for (uint32_t i = 0; i < LLG_WAVE_QUEUE_CAP; i++)
        event_destroy(&g_wave.queue[i]);
    for (uint32_t i = 0; i < g_wave.reg_count; i++) free(g_wave.regs[i].name);
    free(g_wave.regs);
    free(g_wave.map);
    mutex_destroy(&g_wave.mutex);
    cond_destroy(&g_wave.not_empty);
    cond_destroy(&g_wave.not_full);
    cond_destroy(&g_wave.ack_changed);
    memset(&g_wave, 0, sizeof(g_wave));
}

int llg_wave_model_init(uint64_t precision_fs) {
    if (g_wave.initialized) {
        fprintf(stderr, "llg: waveform: model initialized twice without close\n");
        return -1;
    }
    memset(&g_wave, 0, sizeof(g_wave));
    mutex_init(&g_wave.mutex);
    cond_init(&g_wave.not_empty);
    cond_init(&g_wave.not_full);
    cond_init(&g_wave.ack_changed);
    g_wave.precision_fs = precision_fs ? precision_fs : 1u;
    g_wave.producer = thread_self();
    g_wave.initialized = 1;
    return 0;
}

static int register_value(const char* name, void* ptr, uint32_t width,
                          int is_real) {
    if (!require_producer("registration")) return -1;
    if (g_wave.worker_started) {
        wave_error("signal `%s` registered after waveform writer startup",
                   name ? name : "(null)");
        return -1;
    }
    if (!valid_hierarchy_name(name) || !ptr ||
        (!is_real && (width == 0 || width > LLG_MAX_WIDTH))) {
        wave_error("invalid waveform registration");
        return -1;
    }
    for (uint32_t i = 0; i < g_wave.reg_count; i++) {
        if (strcmp(g_wave.regs[i].name, name) == 0) {
            wave_error("duplicate waveform name `%s`", name);
            return -1;
        }
    }
    if (g_wave.reg_count == g_wave.reg_cap) {
        uint32_t cap = g_wave.reg_cap ? g_wave.reg_cap * 2u : 64u;
        registration_t* regs = (registration_t*)realloc(
            g_wave.regs, (size_t)cap * sizeof(registration_t));
        if (!regs) {
            wave_error("out of memory while registering `%s`", name);
            return -1;
        }
        g_wave.regs = regs;
        g_wave.reg_cap = cap;
    }
    registration_t* reg = &g_wave.regs[g_wave.reg_count++];
    reg->name = wave_strdup(name);
    reg->ptr = ptr;
    reg->width = width;
    reg->is_real = (uint8_t)is_real;
    reg->next_alias = LLG_WAVE_NO_REG;
    reg->selected = 1;
    if (!reg->name) {
        g_wave.reg_count--;
        wave_error("out of memory while registering `%s`", name);
        return -1;
    }
    return 0;
}

int llg_wave_register_sv4(const char* name, sv4_t* value, uint32_t width) {
    return register_value(name, value, width, 0);
}

int llg_wave_register_real(const char* name, double* value) {
    return register_value(name, value, 64u, 1);
}

void llg_wave_file(const char* path, uint64_t now) {
    if (!require_producer("$dumpfile") || !path || !start_worker()) return;
    wave_event_t event = {0};
    event.kind = EV_FILE;
    event.now = now;
    size_t len = strlen(path);
    if (len >= sizeof(event.payload.path)) {
        wave_error("$dumpfile path exceeds %u bytes", LLG_WAVE_PATH_CAP - 1u);
        return;
    }
    memcpy(event.payload.path, path, len + 1u);
    queue_push(&event);
}

void llg_wave_dumpvars(uint64_t now) {
    llg_wave_dumpvars_select(now, 0, NULL, 0);
}

void llg_wave_dumpvars_select(uint64_t now, uint32_t depth,
                              const char* const* names, uint32_t name_count) {
    if (!require_producer("$dumpvars")) return;
    if (name_count && !names) {
        wave_error("$dumpvars selection list is null");
        return;
    }
    select_registrations(depth, names, name_count);
    g_wave.producer_dumping = 1;
    enqueue_snapshot(now, SNAP_DUMPVARS);
}

void llg_wave_on(uint64_t now) {
    if (!require_producer("$dumpon")) return;
    g_wave.producer_dumping = 1;
    enqueue_snapshot(now, SNAP_ON);
}

void llg_wave_off(uint64_t now) {
    if (!require_producer("$dumpoff") || !start_worker()) return;
    g_wave.producer_dumping = 0;
    enqueue_simple(EV_OFF, now, 0);
}

void llg_wave_dumpall(uint64_t now) {
    if (!require_producer("$dumpall")) return;
    enqueue_snapshot(now, SNAP_ALL);
}

void llg_wave_flush(uint64_t now) {
    if (!require_producer("$dumpflush") || !start_worker()) return;
    uint64_t sequence = atomic_u64_load(&g_wave.ack) + 1u;
    enqueue_simple(EV_FLUSH, now, sequence);
    mutex_lock(&g_wave.mutex);
    while (atomic_u64_load(&g_wave.ack) < sequence &&
           atomic_int_load(&g_wave.worker_alive))
        cond_wait(&g_wave.ack_changed, &g_wave.mutex);
    mutex_unlock(&g_wave.mutex);
}

void llg_wave_limit(uint64_t bytes, uint64_t now) {
    if (!require_producer("$dumplimit") || !start_worker()) return;
    enqueue_simple(EV_LIMIT, now, bytes);
}

void llg_wave_changed_sv4(sv4_t* ptr, const sv4_t* value, uint64_t now) {
    if (!g_wave.initialized || !g_wave.worker_started ||
        !g_wave.producer_dumping || !require_producer("signal change")) return;
    uint32_t first = lookup_registration(ptr);
    if (first == LLG_WAVE_NO_REG) return;
    wave_event_t event = {0};
    event.kind = EV_CHANGE_SV4;
    event.now = now;
    event.first_reg = first;
    if (!capture_sv4(&event, value)) return;
    queue_push(&event);
}

void llg_wave_changed_real(double* ptr, double value, uint64_t now) {
    if (!g_wave.initialized || !g_wave.worker_started ||
        !g_wave.producer_dumping || !require_producer("real change")) return;
    uint32_t first = lookup_registration(ptr);
    if (first == LLG_WAVE_NO_REG) return;
    wave_event_t event = {0};
    event.kind = EV_CHANGE_REAL;
    event.now = now;
    event.first_reg = first;
    event.payload.real = value;
    queue_push(&event);
}

int llg_wave_close(uint64_t now) {
    if (!g_wave.initialized) return 0;
    if (!require_producer("waveform close")) return -1;
    if (g_wave.worker_started) {
        enqueue_simple(EV_CLOSE, now, 0);
        join_worker();
    }
    int result = atomic_int_load(&g_wave.error) ? -1 : 0;
    free_state();
    return result;
}
