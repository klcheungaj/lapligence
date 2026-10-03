// Counts exclusive transfers in the waveform tests, using the production code.
#include "llg_value.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdatomic.h>

static atomic_size_t allocated;
static atomic_size_t released;

// Model owners are released by the producer while snapshot owners may be
// released concurrently by the writer. Counters are therefore atomic.
size_t storage_test_allocated(void) { return atomic_load(&allocated); }
size_t storage_test_released(void) { return atomic_load(&released); }

static void* snapshot_malloc(size_t bytes) {
    void* pointer = malloc(bytes);
    if (pointer) atomic_fetch_add(&allocated, 1);
    return pointer;
}

static void snapshot_free(void* pointer) {
    if (pointer) atomic_fetch_add(&released, 1);
    free(pointer);
}

#if LLG_SV4_USE_GMP
static void* snapshot_calloc(size_t count, size_t bytes) {
    void* pointer = calloc(count, bytes);
    if (pointer) atomic_fetch_add(&allocated, 1);
    return pointer;
}

// A reallocation keeps one live owner allocation; only a fresh block counts.
static void* snapshot_realloc(void* pointer, size_t bytes) {
    void* moved = realloc(pointer, bytes);
    if (moved && !pointer) atomic_fetch_add(&allocated, 1);
    return moved;
}

// The selected compact units are separate translation units compiled with
// malloc/calloc/realloc/free renamed to these hooks (see CMakeLists.txt).
void* llg_value_test_malloc(size_t bytes) { return snapshot_malloc(bytes); }
void* llg_value_test_calloc(size_t count, size_t bytes) { return snapshot_calloc(count, bytes); }
void* llg_value_test_realloc(void* pointer, size_t bytes) { return snapshot_realloc(pointer, bytes); }
void llg_value_test_free(void* pointer) { snapshot_free(pointer); }
#include "value/destinations.c"
#else
#define malloc snapshot_malloc
#define free snapshot_free
#include "value/storage.c"
#undef free
#undef malloc
#endif
