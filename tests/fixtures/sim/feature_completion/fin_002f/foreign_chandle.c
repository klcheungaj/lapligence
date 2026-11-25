/* The C side keeps its own record of what it freed. */
#include "svdpi.h"
#include <stdint.h>
#include <stdlib.h>

#define OBJECTS 16

static void* objects[OBJECTS];
static int live[OBJECTS];
static int count;

DPI_DLLESPEC void* c_new(void) {
    objects[count] = malloc(8);
    live[count] = 1;
    return objects[count++];
}

/* 0 null, 1 live, 2 freed, 3 unknown: compares the pointer, reads nothing. */
DPI_DLLESPEC int c_state(void* handle) {
    if (!handle) return 0;
    for (int i = count - 1; i >= 0; i--)
        if (objects[i] == handle) return live[i] ? 1 : 2;
    return 3;
}

/* The memory stays with the program, so that no later object has its address. */
DPI_DLLESPEC void c_free(void* handle) {
    for (int i = 0; i < count; i++)
        if (objects[i] == handle) live[i] = 0;
}

DPI_DLLESPEC void* c_same(void* handle) { return handle; }

DPI_DLLESPEC void c_swap(void** a, void** b) {
    void* kept = *a;
    *a = *b;
    *b = kept;
}

DPI_DLLESPEC void* c_wild(void) { return (void*)(uintptr_t)16; }
