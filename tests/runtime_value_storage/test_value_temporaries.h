#ifndef LLG_TEST_VALUE_TEMPORARIES_H
#define LLG_TEST_VALUE_TEMPORARIES_H
#include "llg_value.h"
#include <stdio.h>
#include <stdlib.h>

/* Test expression owners only. Returned descriptors are borrowed, never mutable
 * runtime targets or transferred callback results. Drain after each vector test;
 * this is not the simulator's temporary storage or a production lifetime API. */
typedef struct test_value_owner {
    sv4_t value;
    struct test_value_owner* next;
} test_value_owner_t;
static test_value_owner_t* test_value_owners;

static void test_values_clear(void) {
    while (test_value_owners) {
        test_value_owner_t* owner = test_value_owners;
        test_value_owners = owner->next;
        sv4_destroy(&owner->value);
        free(owner);
    }
}

static sv4_t test_value(sv4_t value) {
    test_value_owner_t* owner = (test_value_owner_t*)malloc(sizeof(*owner));
    if (!owner) {
        sv4_destroy(&value);
        test_values_clear();
        fputs("test value owner allocation failed\n", stderr);
        exit(2);
    }
    owner->value = (sv4_t)SV4_EMPTY;
    sv4_move(&owner->value, &value);
    owner->next = test_value_owners;
    test_value_owners = owner;
    return owner->value;
}

static int test_values_run(int (*probe)(void)) {
    int result = probe();
    test_values_clear();
#ifdef LLG_SELFTEST_TRACK_STORAGE
    extern size_t value_test_live(void);
    if (value_test_live() != 0) {
        fprintf(stderr, "vector leaked %zu packed owners\n", value_test_live());
        return 1;
    }
#endif
    return result;
}
#endif
