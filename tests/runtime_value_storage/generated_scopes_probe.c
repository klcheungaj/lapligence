/* P05 ownership patterns, exercised without switching coroutine stacks.
 * This is a hand-authored output-shape probe, not Rust-emitter integration. */
#include "llg_rt.c"
#include "probe.h"

static void never_run(llg_proc_t* self) { (void)self; abort(); }

static void check_detached_cells(void) {
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    sv4_t first = sv4_from_u64(19, 65, 0);
    sv4_t second = sv4_from_u64(29, 65, 0);
    sv4_t mask = sv4_from_u64(15, 65, 0);
    for (unsigned iteration = 0; iteration < 1000; ++iteration) {
        llg_value_scope_t* mark = llg_value_scope_mark();
        llg_value_scope_t* owner = llg_value_scope_begin(1);
        sv4_t* target = llg_value_scope_values(owner);
        sv4_replace(target, sv4_from_u64(160, 65, 0));
        llg_nba_masked(target, first, mask, 1);
        llg_nba_after(target, second, 2);
        CHECK(owner->references == 3);
        llg_value_scopes_end_since(mark);
        CHECK(!owner->active && owner->owner == NULL && owner->references == 2);
        /* A subsequent lexical iteration must not reuse a retained cell. */
        llg_value_scope_t* other = llg_value_scope_begin(1);
        CHECK(llg_value_scope_values(other) != target);
        sv4_replace(llg_value_scope_values(other), sv4_zero(1, 0));
        llg_value_scope_end(other);
        ++g.now;
        commit_nbas(LLG_REGION_NBA);
        expect_number(sv4_clone(target), 163);
        CHECK(owner->references == 1);
        ++g.now;
        commit_nbas(LLG_REGION_NBA);
        CHECK(all_value_scopes == NULL && root_value_scopes == NULL);
        CHECK(value_test_live() == 3);
    }
    sv4_destroy(&first); sv4_destroy(&second); sv4_destroy(&mask);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
}

static void check_clocking_handoff(void) {
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    sv4_t clock = sv4_zero(1, 0);
    sv4_t one = sv4_from_u64(1, 1, 0);
    sv4_t value = sv4_from_u64(71, 129, 0);
    llg_wait_src_t spec = {.sig = &clock, .kind = LLG_EV_POSEDGE};
    llg_value_scope_t* owner = llg_value_scope_begin(1);
    sv4_t* target = llg_value_scope_values(owner);
    sv4_replace(target, sv4_zero(129, 0));
    llg_clocking_nba_sync_after(target, value, 1, &spec, 1);
    CHECK(g.clocking_drives != NULL && owner->references == 2);
    llg_value_scope_end(owner);
    CHECK(owner->references == 1);
    /* Handoff retains the detached owner before freeing the clocking record. */
    llg_ba(&clock, one);
    CHECK(g.clocking_drives == NULL && g.delayed_nbas != NULL);
    CHECK(owner->references == 1 && !owner->active);
    ++g.now;
    commit_nbas(LLG_REGION_RE_NBA);
    CHECK(all_value_scopes == NULL);
    sv4_destroy(&clock); sv4_destroy(&one); sv4_destroy(&value);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
}

static void check_process_cancellation_and_reinit(void) {
    for (unsigned cycle = 0; cycle < 40; ++cycle) {
        llg_rt_init();
        g.current_region = LLG_REGION_ACTIVE;
        sv4_t value = sv4_from_u64(33, 65, 0);
        llg_proc_t* process = llg_spawn(never_run, "scoped delayed target");
        aco_gtls_co = process->co;
        llg_value_scope_t* outer = llg_value_scope_begin(1);
        sv4_replace(llg_value_scope_values(outer), sv4_zero(65537, 0));
        llg_value_scope_t* marker = llg_value_scope_mark();
        llg_value_scope_t* inner = llg_value_scope_begin(1);
        sv4_t* target = llg_value_scope_values(inner);
        sv4_replace(target, sv4_zero(65, 0));
        llg_nba_after(target, value, 100);
        llg_value_scopes_end_since(marker);
        CHECK(inner->references == 1 && !inner->active);
        CHECK(process->value_scopes == outer);
        aco_gtls_co = g.main_co;
        llg_kill_proc_tree(process);
        reap_retired_procs();
        llg_rt_cleanup();
        llg_rt_cleanup();
        CHECK(all_value_scopes == NULL && root_value_scopes == NULL);
        sv4_destroy(&value);
        CHECK(value_test_live() == 0 && value_test_bytes() == 0);
    }
}

static void check_net_region_and_mask(void) {
    llg_rt_init();
    g.current_region = LLG_REGION_ACTIVE;
    sv4_t driver = sv4_zero(65, 0);
    sv4_t value = sv4_from_u64(170, 65, 0);
    sv4_t mask = sv4_from_u64(15, 65, 0);
    llg_net_t net = {.resolved = SV4_EMPTY, .width = 65, .n_drivers = 1,
        .drivers = {&driver}, .strength0 = {6}, .strength1 = {6}};
    sv4_replace(&net.resolved, sv4_zero(65, 0));
    llg_nba_net_after(&net, 0, value, 1);
    value.bits[0] = 85;
    ++g.now;
    commit_nbas(LLG_REGION_RE_NBA); /* ordinary NBA is not re-NBA */
    expect_number(sv4_clone(&driver), 0);
    commit_nbas(LLG_REGION_NBA);
    expect_number(sv4_clone(&driver), 170);
    expect_number(sv4_clone(&net.resolved), 170);
    llg_nba_net_masked_after(&net, 0, value, mask, 1);
    value.bits[0] = 0;
    ++g.now;
    commit_nbas(LLG_REGION_NBA);
    expect_number(sv4_clone(&driver), 165);
    expect_number(sv4_clone(&net.resolved), 165);
    llg_rt_cleanup();
    sv4_destroy(&driver); sv4_destroy(&net.resolved);
    sv4_destroy(&value); sv4_destroy(&mask);
    CHECK(value_test_live() == 0);
}

/* Returned owners are transferred into the caller's registered slot before
 * any subsequent yielding call. Callee arguments borrow; inputs are cloned. */
static sv4_t recursive_value(sv4_t argument, unsigned depth) {
    llg_value_scope_t* base = llg_value_scope_mark();
    sv4_t* values = llg_value_scope_values(llg_value_scope_begin(3));
    sv4_copy(&values[0], &argument);
    if (depth == 0) sv4_replace(&values[1], sv4_from_u64(1, 65, 0));
    else {
        sv4_replace(&values[2], recursive_value(values[0], depth - 1));
        sv4_replace(&values[1], sv4_add(values[0], values[2]));
        sv4_destroy(&values[2]);
    }
    sv4_t returned = sv4_clone(&values[1]);
    llg_value_scopes_end_since(base);
    return returned;
}

static void check_expression_temporaries(void) {
    llg_rt_init();
    llg_value_scope_t* base = llg_value_scope_mark();
    sv4_t* values = llg_value_scope_values(llg_value_scope_begin(3));
    sv4_replace(&values[0], sv4_from_u64(7, 65, 0));
    for (unsigned iteration = 0; iteration < 2000; ++iteration) {
        llg_value_scope_t* block = llg_value_scope_mark();
        sv4_replace(&values[1], recursive_value(values[0], 8));
        expect_number(sv4_clone(&values[1]), 57);
        sv4_replace(&values[2], sv4_x(1, 0));
        sv4_replace(&values[2], sv4_mux(values[2], values[0], values[1]));
        sv4_destroy(&values[1]); sv4_destroy(&values[2]);
        llg_value_scopes_end_since(block);
        CHECK(value_test_live() == 1);
    }
    llg_value_scopes_end_since(base);
    CHECK(all_value_scopes == NULL);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0 && value_test_bytes() == 0);
}

int main(void) {
    check_detached_cells();
    check_clocking_handoff();
    check_process_cancellation_and_reinit();
    check_net_region_and_mask();
    check_expression_temporaries();
    puts("P05 scope marks, retained targets, clocking handoff, net NBA and expression patterns: OK");
    return 0;
}
