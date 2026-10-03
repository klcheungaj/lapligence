#ifndef LLG_GMP4_ALLOCATION_HOOKS_H
#define LLG_GMP4_ALLOCATION_HOOKS_H
#include <stddef.h>
void *sv4_test_alloc(size_t size);
void sv4_test_free(void *pointer);
#define GMP4_ALLOC sv4_test_alloc
#define GMP4_FREE sv4_test_free
#endif
