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

#define malloc snapshot_malloc
#define free snapshot_free
#include "value/storage.c"
#undef free
#undef malloc
