// llg_alloc_fault.h: allocation fault injection for generated models.
//
// A test build only: llg_msg.h includes this header when LLG_ALLOC_FAULT is
// defined (`--cflags -DLLG_ALLOC_FAULT`), and nothing of it exists in any
// other build. It then renames every malloc, calloc and realloc call of the
// runtime and of the generated model to a counting wrapper, so one built
// model can be run once per allocation request with that request failing:
//
//   LLG_ALLOC_FAULT=<n>         request n (counted from 1, all three calls
//                               together, in call order) returns NULL
//   LLG_ALLOC_FAULT=<n>+        request n and every later one return NULL
//   LLG_ALLOC_FAULT_COUNT=<f>   a run that ends through exit() writes the
//                               number of requests it made to file <f>
//
// Without LLG_ALLOC_FAULT in the environment nothing fails. What the wrapper
// does not see is listed in tests/readme.md ("Allocation fault injection").
#ifndef LLG_ALLOC_FAULT_H
#define LLG_ALLOC_FAULT_H

#include <stddef.h>
#include <stdlib.h>

void* llg_alloc_fault_malloc(size_t bytes);
void* llg_alloc_fault_calloc(size_t count, size_t size);
void* llg_alloc_fault_realloc(void* old, size_t bytes);

#if defined(LLG_MSG_IMPLEMENTATION)
#include "llg_platform.h"

#include <inttypes.h>
#include <stdio.h>

// 0: environment not read, 1: being read, 2: ready.
static llg_atomic_int_t llg_alloc_fault_state;
static llg_atomic_u64_t llg_alloc_fault_requests;
static uint64_t llg_alloc_fault_target;
static int32_t llg_alloc_fault_sticky;
static const char* llg_alloc_fault_count_path;

static void llg_alloc_fault_write_count(void) {
    FILE* out = fopen(llg_alloc_fault_count_path, "w");
    if (!out) return;
    fprintf(out, "%" PRIu64 "\n", llg_atomic_u64_load(&llg_alloc_fault_requests));
    fclose(out);
}

static void llg_alloc_fault_configure(void) {
    const char* text = getenv("LLG_ALLOC_FAULT");
    if (text && *text) {
        char* end = NULL;
        llg_alloc_fault_target = (uint64_t)strtoull(text, &end, 10);
        llg_alloc_fault_sticky = end && *end == '+';
    }
    const char* path = getenv("LLG_ALLOC_FAULT_COUNT");
    if (path && *path) {
        llg_alloc_fault_count_path = path;
        atexit(llg_alloc_fault_write_count);
    }
}

// Whether this request is the one (or one of those) chosen to fail. The
// waveform writer thread allocates too, so the count is atomic and the
// environment is read by exactly one thread.
static int32_t llg_alloc_fault_fails(void) {
    while (llg_atomic_int_load(&llg_alloc_fault_state) != 2) {
        if (llg_atomic_int_cas(&llg_alloc_fault_state, 0, 1)) {
            llg_alloc_fault_configure();
            llg_atomic_int_store(&llg_alloc_fault_state, 2);
        }
    }
    const uint64_t request = llg_atomic_u64_fetch_add(&llg_alloc_fault_requests, 1) + 1;
    if (llg_alloc_fault_target == 0) return 0;
    return llg_alloc_fault_sticky ? request >= llg_alloc_fault_target
                                  : request == llg_alloc_fault_target;
}

// A parenthesized name is not a function-like macro call, so these reach the
// C library whether or not the renaming macros below are already defined.
void* llg_alloc_fault_malloc(size_t bytes) {
    return llg_alloc_fault_fails() ? NULL : (malloc)(bytes);
}

void* llg_alloc_fault_calloc(size_t count, size_t size) {
    return llg_alloc_fault_fails() ? NULL : (calloc)(count, size);
}

void* llg_alloc_fault_realloc(void* old, size_t bytes) {
    return llg_alloc_fault_fails() ? NULL : (realloc)(old, bytes);
}
#endif

#define malloc(bytes) llg_alloc_fault_malloc(bytes)
#define calloc(count, size) llg_alloc_fault_calloc(count, size)
#define realloc(old, bytes) llg_alloc_fault_realloc(old, bytes)

#endif
