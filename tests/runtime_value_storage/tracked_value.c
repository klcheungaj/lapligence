// Instrument every allocation in the production value module. Other runtime
// allocations remain visible to ASan/LSan; this counter checks value plateaus.
#include "llg_value.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stddef.h>

typedef union {
    max_align_t alignment;
    size_t bytes;
} allocation_header_t;
static size_t live_allocations;
static size_t live_bytes;
size_t value_test_live(void) { return live_allocations; }
size_t value_test_bytes(void) { return live_bytes; }

static void* value_malloc(size_t bytes) {
    if (bytes > SIZE_MAX - sizeof(allocation_header_t)) return NULL;
    allocation_header_t* header = malloc(sizeof(*header) + bytes);
    if (!header) return NULL;
    header->bytes = bytes;
    ++live_allocations;
    live_bytes += bytes;
    return header + 1;
}
static void value_free(void* pointer) {
    if (!pointer) return;
    allocation_header_t* header = (allocation_header_t*)pointer - 1;
    if (!live_allocations || live_bytes < header->bytes) abort();
    --live_allocations;
    live_bytes -= header->bytes;
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
