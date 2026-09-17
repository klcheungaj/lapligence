/* Exercise address-index growth, tombstones, retained lexical scopes, globals,
 * empty scopes and repeated teardown without relying on wall-clock timings. */
#include "llg_rt.c"
#include "probe.h"

static void exercise_index(void) {
    const size_t count = 1024;
    llg_value_scope_t** scopes = calloc(count, sizeof(*scopes));
    CHECK(scopes != NULL);
    sv4_t global = sv4_zero(65, 0);
    sv4_t rhs = sv4_from_u64(7, 65, 0);
    llg_rt_init();
    for (size_t i = 0; i < count; ++i) {
        scopes[i] = llg_value_scope_begin(i % 17);
        for (size_t j = 0; j < scopes[i]->count; ++j) {
            sv4_t* target = &scopes[i]->values[j];
            sv4_copy(target, &rhs);
            CHECK(value_scope_index_find(target) == scopes[i]);
        }
    }
    CHECK(value_scope_retain_target(&global) == NULL);
    CHECK(value_scope_retain_target(NULL) == NULL);
    /* Non-LIFO scope exit, then growth must preserve every surviving key. */
    for (size_t i = 0; i < count; i += 2) {
        llg_value_scope_end(scopes[i]);
        scopes[i] = llg_value_scope_begin((i % 19) + 1);
    }
    for (size_t i = 0; i < count; ++i) {
        for (size_t j = 0; j < scopes[i]->count; ++j) {
            sv4_t* target = &scopes[i]->values[j];
            CHECK(value_scope_index_find(target) == scopes[i]);
            llg_value_scope_t* retained = value_scope_retain_target(target);
            CHECK(retained == scopes[i]);
            value_scope_release(retained);
        }
    }
    llg_value_scope_t* pending = llg_value_scope_begin(1);
    sv4_t* target = pending->values;
    sv4_copy(target, &rhs);
    llg_nba_after(target, rhs, 1);
    llg_value_scope_end(pending);
    CHECK(!pending->active && pending->references == 1);
    CHECK(value_scope_index_find(target) == pending);
    /* A second queued write can retain an already lexically closed scope. */
    llg_nba_after(target, rhs, 2);
    CHECK(pending->references == 2);
    llg_nba_after(&global, rhs, 3);
    for (size_t i = count; i > 0; --i) llg_value_scope_end(scopes[i - 1]);
    CHECK(value_scope_count == 1);
    g.now = 1;
    commit_nbas(LLG_REGION_NBA);
    CHECK(pending->references == 1 && target->bits[0] == 7);
    g.now = 2;
    commit_nbas(LLG_REGION_NBA);
    /* Do not read target/pending after their final owning NBA is released. */
    CHECK(value_scope_count == 0 && value_scope_index == NULL);
    llg_rt_cleanup();
    CHECK(all_value_scopes == NULL && root_value_scopes == NULL);
    sv4_destroy(&global);
    sv4_destroy(&rhs);
    free(scopes);
    CHECK(value_test_live() == 0);
}

int main(void) {
    for (unsigned cycle = 0; cycle < 4; ++cycle) exercise_index();
    puts("scope address index: OK");
    return 0;
}
