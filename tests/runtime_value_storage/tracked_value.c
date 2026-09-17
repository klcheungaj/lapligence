// Instrument every allocation in the production value module. Other runtime
// allocations remain visible to ASan/LSan; this counter checks value plateaus.
#include "llg_value.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stddef.h>
#include <stdatomic.h>

typedef union {
    max_align_t alignment;
    size_t bytes;
} allocation_header_t;
static size_t live_allocations;
static size_t live_bytes;
static size_t total_allocations;
static size_t peak_bytes;
static size_t peak_allocations;
// Model storage is produced on the simulation thread; waveform snapshots may
// be destroyed concurrently by the writer. Keep multi-counter updates coherent.
static atomic_flag counter_lock = ATOMIC_FLAG_INIT;
static void lock_counters(void) {
    while (atomic_flag_test_and_set_explicit(&counter_lock, memory_order_acquire)) {}
}
static void unlock_counters(void) {
    atomic_flag_clear_explicit(&counter_lock, memory_order_release);
}
static size_t read_counter(const size_t* counter) {
    lock_counters();
    size_t result = *counter;
    unlock_counters();
    return result;
}
size_t value_test_live(void) { return read_counter(&live_allocations); }
size_t value_test_bytes(void) { return read_counter(&live_bytes); }
size_t value_test_allocations(void) { return read_counter(&total_allocations); }
size_t value_test_peak_bytes(void) { return read_counter(&peak_bytes); }
size_t value_test_peak_live(void) { return read_counter(&peak_allocations); }
void value_test_reset_stats(void) {
    lock_counters();
    total_allocations = 0;
    peak_bytes = live_bytes;
    peak_allocations = live_allocations;
    unlock_counters();
}

static void* value_malloc(size_t bytes) {
    lock_counters();
    if (bytes > SIZE_MAX - sizeof(allocation_header_t) ||
        bytes > SIZE_MAX - live_bytes || total_allocations == SIZE_MAX ||
        live_allocations == SIZE_MAX) {
        unlock_counters();
        return NULL;
    }
    allocation_header_t* header = malloc(sizeof(*header) + bytes);
    if (!header) {
        unlock_counters();
        return NULL;
    }
    header->bytes = bytes;
    ++live_allocations;
    live_bytes += bytes;
    ++total_allocations;
    if (live_bytes > peak_bytes) peak_bytes = live_bytes;
    if (live_allocations > peak_allocations) peak_allocations = live_allocations;
    unlock_counters();
    return header + 1;
}
static void value_free(void* pointer) {
    if (!pointer) return;
    allocation_header_t* header = (allocation_header_t*)pointer - 1;
    lock_counters();
    if (!live_allocations || live_bytes < header->bytes) abort();
    --live_allocations;
    live_bytes -= header->bytes;
    unlock_counters();
    free(header);
}
static void* value_calloc(size_t count, size_t bytes) {
    if (bytes && count > SIZE_MAX / bytes) return NULL;
    size_t total = count * bytes;
    void* pointer = value_malloc(total);
    if (pointer) memset(pointer, 0, total);
    return pointer;
}
#define malloc value_malloc
#define calloc value_calloc
#define free value_free
#include "llg_value.c"
#undef free
#undef calloc
#undef malloc
