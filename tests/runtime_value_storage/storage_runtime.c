// Counts exclusive transfers in the waveform tests, using the production code.
#include "llg_value.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static size_t allocated;
static size_t released;

// Only the simulation producer allocates. Only the writer releases while it
// is running; direct queue tests and teardown release after no worker is live.
// The test reads these counters only before startup or after the worker joins.
size_t storage_test_allocated(void) { return allocated; }
size_t storage_test_released(void) { return released; }

static void* snapshot_malloc(size_t bytes) {
    void* pointer = malloc(bytes);
    if (pointer) allocated++;
    return pointer;
}

static void snapshot_free(void* pointer) {
    if (pointer) released++;
    free(pointer);
}

#define malloc snapshot_malloc
#define free snapshot_free
#include "value/storage.c"
#undef free
#undef malloc
