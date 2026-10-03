#ifndef LLG_SV4_PROTOTYPE_TEST_CHECK_H
#define LLG_SV4_PROTOTYPE_TEST_CHECK_H
#include <stdio.h>
#include <stdlib.h>
#define CHECK(condition) do { if (!(condition)) { \
    fprintf(stderr, "CHECK failed: %s at %s:%d\n", #condition, __FILE__, __LINE__); \
    exit(1); \
} } while (0)
#endif
