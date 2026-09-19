/* Production select-plan checks against an independent per-bit address oracle.
 * The oracle retains an explicit map, not the runtime's interval algebra. */
#include "probe.h"
#include <limits.h>
#include <string.h>

#define ORACLE_BITS 256
static unsigned state_at(sv4_t value, unsigned bit) {
    uint64_t mask = UINT64_C(1) << (bit % 64);
    unsigned limb = bit / 64;
    if (value.x[limb] & mask) return 2;
    if (value.z[limb] & mask) return 3;
    return (value.bits[limb] & mask) != 0;
}
static void put_state(sv4_t* value, unsigned bit, unsigned state) {
    uint64_t mask = UINT64_C(1) << (bit % 64);
    unsigned limb = bit / 64;
    value->bits[limb] &= ~mask;
    value->x[limb] &= ~mask;
    value->z[limb] &= ~mask;
    if (state == 1) value->bits[limb] |= mask;
    if (state == 2) value->x[limb] |= mask;
    if (state == 3) value->z[limb] |= mask;
}
static sv4_t pattern(unsigned width, unsigned seed) {
    sv4_t value = sv4_zero(width, 0);
    for (unsigned bit = 0; bit < width; ++bit)
        put_state(&value, bit, (bit * 13 + seed + bit / 3) % 4);
    return value;
}
static void step(sv4_select_plan_t* plan, int64_t base, unsigned width) {
    sv4_t index = sv4_from_i64(base, 64);
    sv4_select_plan_step(plan, index, width);
    sv4_destroy(&index);
}
static void oracle_step(int* map, unsigned* old_width, int64_t base, unsigned width) {
    int next[ORACLE_BITS];
    CHECK(width <= ORACLE_BITS);
    for (unsigned bit = 0; bit < width; ++bit) {
        /* Test-generated bases are small; endpoint tests are separate. */
        int64_t index = base + (int64_t)bit;
        next[bit] = index >= 0 && index < *old_width ? map[index] : -1;
    }
    memcpy(map, next, width * sizeof(*map));
    *old_width = width;
}
static void check_value_and_store(sv4_t source, const sv4_select_plan_t* plan, const int* map) {
    sv4_t read = sv4_select_plan_read(source, plan);
    sv4_t target = sv4_clone(&source);
    sv4_t rhs = pattern(plan->width, 3);
    unsigned expected[ORACLE_BITS];
    CHECK(read.width == plan->width && !read.is_signed && read.bits != source.bits);
    for (unsigned bit = 0; bit < source.width; ++bit) expected[bit] = state_at(source, bit);
    for (unsigned bit = 0; bit < plan->width; ++bit) {
        unsigned want = map[bit] < 0 ? 2 : state_at(source, (unsigned)map[bit]);
        CHECK(state_at(read, bit) == want);
        if (map[bit] >= 0) expected[map[bit]] = state_at(rhs, bit);
    }
    sv4_select_plan_set(&target, plan, rhs);
    for (unsigned bit = 0; bit < source.width; ++bit) CHECK(state_at(target, bit) == expected[bit]);
    sv4_destroy(&read);
    sv4_destroy(&target);
    sv4_destroy(&rhs);
}
static void matrix(void) {
    sv4_t source = pattern(6, 1);
    size_t cases = 0;
    for (int first = -5; first <= 8; ++first) {
        for (unsigned first_width = 1; first_width <= 6; ++first_width) {
            for (int second = -5; second <= 8; ++second) {
                for (unsigned width = 1; width <= 6; ++width) {
                    int map[ORACLE_BITS];
                    unsigned old_width = source.width;
                    for (unsigned bit = 0; bit < old_width; ++bit) map[bit] = (int)bit;
                    sv4_select_plan_t plan = sv4_select_plan_init(source.width);
                    step(&plan, first, first_width);
                    oracle_step(map, &old_width, first, first_width);
                    step(&plan, second, width);
                    oracle_step(map, &old_width, second, width);
                    check_value_and_store(source, &plan, map);
                    /* Further refinement must not resurrect an invalid prefix. */
                    step(&plan, -1, 4);
                    oracle_step(map, &old_width, -1, 4);
                    check_value_and_store(source, &plan, map);
                    CHECK(value_test_live() == 1);
                    ++cases;
                }
            }
        }
    }
    sv4_destroy(&source);
    CHECK(cases == 7056 && value_test_live() == 0);
    printf("packed map oracle: %zu chains, read and write after two and three steps\n", cases);
}
static void wide_and_alias(void) {
    sv4_t source = pattern(129, 0);
    int map[ORACLE_BITS];
    unsigned width = source.width;
    for (unsigned bit = 0; bit < width; ++bit) map[bit] = (int)bit;
    sv4_select_plan_t plan = sv4_select_plan_init(width);
    step(&plan, 61, 65); oracle_step(map, &width, 61, 65);
    step(&plan, -3, 71); oracle_step(map, &width, -3, 71);
    step(&plan, 1, 69); oracle_step(map, &width, 1, 69);
    check_value_and_store(source, &plan, map);
    sv4_destroy(&source);
    for (int offset = -7; offset <= 7; ++offset) {
        sv4_t target = pattern(129, 1);
        sv4_t snapshot = sv4_clone(&target);
        plan = sv4_select_plan_init(129);
        step(&plan, offset, 129);
        /* Same owner borrowed as RHS and destination; a forward bit loop alone
         * would corrupt the source when offset > 0. */
        sv4_select_plan_set(&target, &plan, target);
        for (unsigned bit = 0; bit < 129; ++bit) {
            int from = (int)bit - offset;
            unsigned expected = state_at(snapshot, from >= 0 && from < 129 ? (unsigned)from : bit);
            CHECK(state_at(target, bit) == expected);
        }
        sv4_destroy(&snapshot);
        sv4_destroy(&target);
        CHECK(value_test_live() == 0);
    }
}
static void reported_examples(void) {
    sv4_t source = sv4_zero(16, 0);
    sv4_t rhs = sv4_from_u64(7, 3, 0);
    sv4_select_plan_t plan = sv4_select_plan_init(16);
    step(&plan, 8, 8); step(&plan, 3, 3);
    sv4_select_plan_set(&source, &plan, rhs);
    CHECK(sv4_to_u64(source) == 0x3800); /* R03: ascending [2 +: 3]. */
    sv4_replace(&source, sv4_from_u64(0xa500, 16, 0));
    sv4_replace(&rhs, sv4_from_u64(15, 4, 0));
    plan = sv4_select_plan_init(16);
    step(&plan, 0, 8); step(&plan, 6, 4);
    sv4_select_plan_set(&source, &plan, rhs);
    CHECK(sv4_to_u64(source) == 0xa5c0); /* R04: no adjacent-lane write. */
    sv4_t read = sv4_select_plan_read(source, &plan);
    CHECK(read.bits[0] == 3 && read.x[0] == 12 && !read.z[0]);
    sv4_destroy(&read);
    sv4_replace(&source, sv4_from_u64(UINT64_C(0x1122334455667788), 64, 0));
    sv4_replace(&rhs, sv4_from_u64(0xbeef, 16, 0));
    plan = sv4_select_plan_init(64);
    step(&plan, 32, 32); step(&plan, 8, 16);
    expect_number(sv4_select_plan_read(source, &plan), 0x2233);
    sv4_select_plan_set(&source, &plan, rhs);
    CHECK(sv4_to_u64(source) == UINT64_C(0x11beef4455667788)); /* R05: all 16 bits. */
    sv4_destroy(&rhs);
    sv4_destroy(&source);
    CHECK(value_test_live() == 0);
}
static void invalid_indices(void) {
    sv4_t source = sv4_zero(16, 0);
    sv4_t rhs = sv4_from_u64(7, 3, 0);
    for (unsigned mode = 0; mode < 5; ++mode) {
        sv4_t index;
        if (mode == 0) index = sv4_x(64, 1);
        else if (mode == 1) index = sv4_fill(3, 64, 1);
        else if (mode == 2) index = sv4_from_i64(INT64_MIN, 64);
        else if (mode == 3) index = sv4_from_i64(INT64_MAX, 64);
        else {
            index = sv4_zero(129, 0);
            index.bits[2] = 1; /* Must not truncate a wide index to zero. */
        }
        sv4_select_plan_t plan = sv4_select_plan_init(16);
        sv4_select_plan_step(&plan, index, 8);
        step(&plan, 0, 3);
        sv4_t read = sv4_select_plan_read(source, &plan);
        CHECK(read.width == 3 && read.x[0] == 7);
        sv4_select_plan_set(&source, &plan, rhs);
        CHECK(source.bits[0] == 0 && !sv4_is_unknown(source));
        sv4_destroy(&read);
        sv4_destroy(&index);
    }
    sv4_destroy(&rhs);
    sv4_destroy(&source);
    CHECK(value_test_live() == 0);
}
int main(int argc, char** argv) {
    if (argc > 1) {
        sv4_select_plan_t plan = sv4_select_plan_init(16);
        if (!strcmp(argv[1], "zero")) (void)sv4_select_plan_init(0);
        else if (!strcmp(argv[1], "storage")) {
            sv4_t short_value = sv4_zero(8, 0);
            sv4_t read = sv4_select_plan_read(short_value, &plan);
            sv4_destroy(&read); sv4_destroy(&short_value);
        } else if (!strcmp(argv[1], "value")) {
            sv4_t target = sv4_zero(16, 0), rhs = sv4_zero(8, 0);
            sv4_select_plan_set(&target, &plan, rhs);
            sv4_destroy(&target); sv4_destroy(&rhs);
        } else return 2;
        return 0;
    }
    matrix(); wide_and_alias(); reported_examples(); invalid_indices();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    puts("packed selection ownership, clipping, width and index checks passed");
    return 0;
}
