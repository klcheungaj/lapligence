#ifndef LLG_OWNER_PROBE_H
#define LLG_OWNER_PROBE_H
#include <stdio.h>
#include <stdlib.h>
#include "llg_value.h"
size_t value_test_live(void);
size_t value_test_bytes(void);
size_t value_test_allocations(void);
size_t value_test_peak_bytes(void);
size_t value_test_peak_live(void);
void value_test_reset_stats(void);
#define CHECK(condition) do { \
    if (!(condition)) { \
        fprintf(stderr, "ownership check failed at %s:%d: %s (live=%zu bytes=%zu)\n", \
                __FILE__, __LINE__, #condition, value_test_live(), value_test_bytes()); \
        exit(2); \
    } \
} while (0)
static inline void expect_number(sv4_t owned, uint64_t number) {
    CHECK(!sv4_is_unknown(owned));
    CHECK(sv4_to_u64(owned) == number);
    sv4_destroy(&owned);
}
#endif
