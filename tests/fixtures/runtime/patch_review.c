/* Regressions at the public generated-model/runtime boundary. */
#ifdef NDEBUG
#undef NDEBUG
#endif
#include "llg_rt.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static sv4_t a, b, source;
static double real_target;
static llg_net_t net;
static llg_event_object_t event_object;
static llg_event_t event_handle = { &event_object };
static int observed;

static sv4_t v(unsigned value) { return sv4_from_u64(value, 8, 0); }
static unsigned u(sv4_t value) {
    assert(!sv4_is_unknown(value));
    return (unsigned)value.bits[0];
}
static llg_force_part_t part(sv4_t* target, llg_net_t* object, int hi, int lo) {
    llg_force_part_t result = { target, object, hi, lo, (uint32_t)(hi-lo+1), 0, 0 };
    return result;
}
static void eval_ff(sv4_t* out) { *out = v(255); }
static void eval_zero(sv4_t* out) { *out = v(0); }
static void eval_real(double* out) { *out = 3.5; }
static void forced_real_nba(llg_proc_t* self) {
    llg_force_real(&real_target, eval_real, NULL, 0);
    llg_nba_d(&real_target, 9.0);
    llg_nba_d_after(&real_target, 10.0, 1);
    llg_wait_time(2);
    assert(real_target == 3.5);
    llg_release_real(&real_target);
    assert(real_target == 3.5);
    llg_nba_d(&real_target, 7.0);
    llg_wait_time(1);
    assert(real_target == 7.0);
    llg_proc_done(self);
}
static void force_overlap(llg_proc_t* self) {
    llg_force_part_t whole = part(&net.resolved, &net, 7, 0);
    llg_force_part_t low = part(&net.resolved, &net, 3, 0);
    llg_force_expr_parts(&whole, 1, 0, 0, eval_ff, NULL, 0);
    llg_force_expr_parts(&low, 1, 0, 0, eval_zero, NULL, 0);
    assert(u(net.resolved) == 240);
    llg_net_write(&net, 0, v(0x25));
    llg_release_parts(&low, 1, 0, 0);
    assert(u(net.resolved) == 245);
    llg_release_parts(&whole, 1, 0, 0);
    assert(u(net.resolved) == 0x25);
    llg_proc_done(self);
}
static void force_partial_release(llg_proc_t* self) {
    llg_force_part_t whole = part(&net.resolved, &net, 7, 0);
    llg_force_part_t low = part(&net.resolved, &net, 3, 0);
    llg_force_expr_parts(&whole, 1, 0, 0, eval_ff, NULL, 0);
    llg_release_parts(&low, 1, 0, 0);
    assert(u(net.resolved) == 240);
    llg_net_write(&net, 0, v(5));
    assert(u(net.resolved) == 245);
    llg_release_parts(&whole, 1, 0, 0);
    assert(u(net.resolved) == 5);
    llg_proc_done(self);
}
static void force_concat_release(llg_proc_t* self) {
    a = b = sv4_from_u64(0, 4, 0);
    llg_force_part_t parts[2] = {part(&a, NULL, 3, 0), part(&b, NULL, 3, 0)};
    parts[0].value_lsb = 4;
    llg_force_expr_parts(parts, 2, 0, 0, eval_ff, NULL, 0);
    llg_force_part_t single = part(&a, NULL, 3, 0);
    llg_release_parts(&single, 1, 0, 0);
    llg_ba(&a, sv4_from_u64(0, 4, 0));
    llg_ba(&b, sv4_from_u64(0, 4, 0));
    assert(u(a) == 0 && u(b) == 15);
    llg_proc_done(self);
}
static void force_slot_reuse(llg_proc_t* self) {
    llg_force_part_t low = part(&net.resolved, &net, 3, 0);
    llg_force_part_t high = part(&net.resolved, &net, 7, 4);
    llg_force_part_t whole = part(&net.resolved, &net, 7, 0);
    llg_force_expr_parts(&low, 1, 0, 0, eval_zero, NULL, 0);
    llg_force_expr_parts(&high, 1, 0, 0, eval_zero, NULL, 0);
    llg_release_parts(&low, 1, 0, 0);
    llg_force_expr_parts(&whole, 1, 0, 0, eval_ff, NULL, 0);
    assert(u(net.resolved) == 255);
    llg_proc_done(self);
}
static void wait_event(llg_proc_t* self) {
    llg_wait_event(&event_handle);
    observed++;
    llg_proc_done(self);
}
static void finish_later(llg_proc_t* self) {
    (void)self;
    llg_wait_time(1);
    llg_rt_finish();
}
static void self_disabling_child(llg_proc_t* self) {
    (void)self;
    llg_disable_target(1, 1);
    assert(!"disabled branch returned");
    abort();
}
static void nested_child(llg_proc_t* self) {
    llg_fork_group_t* group = llg_fork_group_new(LLG_JOIN);
    llg_fork(self_disabling_child, "nested disabling child", group);
    llg_join(group);
    assert(!"disabled ancestor returned");
    llg_proc_done(self);
}
static int nested;
static void fork_parent(llg_proc_t* self) {
    llg_fork_group_t* group = llg_fork_group_new_target(LLG_JOIN, 1, 1);
    llg_fork(nested ? nested_child : self_disabling_child, "disabling child", group);
    llg_join(group);
    observed++;
    llg_proc_done(self);
}
static void assert_before_design(llg_proc_t* self) {
    llg_wait_any((sv4_t*[]){&b}, 1);
    assert(u(a) == 0);
    observed++;
    llg_proc_done(self);
}
static void active_update(void* data) { (void)data; llg_ba(&a, v(1)); }
static void reactive_writer(llg_proc_t* self) {
    llg_schedule_region_callback(LLG_REGION_ACTIVE, active_update, NULL);
    llg_nba(&b, v(1));
    llg_proc_done(self);
}
static void nba_value_observer(void* data) {
    (void)data;
    assert(u(a) == 1);
    observed++;
}
static void pre_nba_callback(void* data) {
    (void)data;
    llg_schedule_region_callback(LLG_REGION_ACTIVE, active_update, NULL);
}
static void sample_observer(void* data) {
    (void)data;
    sv4_t sample;
    assert(llg_sampled_copy(&a, &sample));
    assert(u(sample) == 0);
    observed++;
}
static void before_postponed(void* data) {
    (void)data;
    llg_ba(&a, v(1));
    llg_schedule_region_callback(LLG_REGION_ACTIVE, sample_observer, NULL);
}
static void nba_then_done(llg_proc_t* self) { llg_nba(&a, v(1)); llg_proc_done(self); }
static void finish_now(llg_proc_t* self) { (void)self; llg_rt_finish(); }
static void monitor_eval(sv4_t* out, void* context) { (void)context; out[0] = a; }
static void monitor_reenable(llg_proc_t* self) {
    llg_monitor_with_reads("value=%0d", 1, monitor_eval, (sv4_t*[]){&a}, 1);
    llg_wait_time(1);
    llg_monitor_set(1);
    llg_wait_time(1);
    llg_proc_done(self);
}
static void event_alias_readonly(void* data) { (void)data; llg_event_assign_null(&event_handle); }
static void illegal_early_spawn(llg_proc_t* self) {
    llg_spawn_in_region(wait_event, "invalid preponed", LLG_REGION_PREPONED);
    llg_proc_done(self);
}
static void callback_nba(void* data) { (void)data; llg_nba(&a, v(1)); }
static void init(void) {
    llg_rt_init();
    a = b = source = v(0);
    real_target = 0;
    observed = 0;
    memset(&net, 0, sizeof(net));
    net.width = 8;
    net.resolved = v(0);
    net.n_drivers = 1;
    net.drivers[0] = &source;
    net.strength0[0] = net.strength1[0] = 6;
    memset(&event_object, 0, sizeof(event_object));
    event_handle.object = &event_object;
}
int main(int argc, char** argv) {
    assert(argc == 2);
    init();
    const char* name = argv[1];
    if (!strcmp(name, "forced_real_nba")) llg_spawn(forced_real_nba, name);
    else if (!strcmp(name, "force_overlap")) llg_spawn(force_overlap, name);
    else if (!strcmp(name, "force_partial_release")) llg_spawn(force_partial_release, name);
    else if (!strcmp(name, "force_concat_release")) llg_spawn(force_concat_release, name);
    else if (!strcmp(name, "force_slot_reuse")) llg_spawn(force_slot_reuse, name);
    else if (!strcmp(name, "event_cleanup")) {
        llg_spawn(wait_event, name);
        llg_spawn(finish_later, "finish");
    } else if (!strcmp(name, "fork_self_disable") || !strcmp(name, "fork_ancestor_disable")) {
        nested = !strcmp(name, "fork_ancestor_disable");
        llg_spawn(fork_parent, name);
    } else if (!strcmp(name, "reactive_fixed_point")) {
        llg_spawn_in_region(assert_before_design, name, LLG_REGION_REACTIVE);
        llg_spawn_in_region(reactive_writer, "reactive writer", LLG_REGION_REACTIVE);
    } else if (!strcmp(name, "pre_nba_reentry")) {
        llg_schedule_region_callback(LLG_REGION_PRE_NBA, pre_nba_callback, NULL);
        llg_schedule_region_callback(LLG_REGION_NBA, nba_value_observer, NULL);
    } else if (!strcmp(name, "preponed_once")) {
        llg_sampled_register(&a);
        llg_schedule_region_callback(LLG_REGION_PRE_POSTPONED, before_postponed, NULL);
    } else if (!strcmp(name, "finish_pending")) {
        llg_spawn(nba_then_done, "NBA producer");
        llg_spawn(finish_now, "finish");
    } else if (!strcmp(name, "monitor_reenable")) llg_spawn(monitor_reenable, name);
    else if (!strcmp(name, "event_readonly"))
        llg_schedule_region_callback(LLG_REGION_OBSERVED, event_alias_readonly, NULL);
    else if (!strcmp(name, "early_spawn")) llg_spawn(illegal_early_spawn, name);
    else if (!strcmp(name, "callback_nba"))
        llg_schedule_region_callback(LLG_REGION_ACTIVE, callback_nba, NULL);
    else { fprintf(stderr, "unknown case: %s\n", name); return 2; }
    llg_rt_run();
    if (!strcmp(name, "event_readonly")) {
        assert(llg_rt_failed());
        assert(event_handle.object == &event_object);
    } else if (!strcmp(name, "early_spawn")) assert(llg_rt_failed());
    else assert(!llg_rt_failed());
    if (!strcmp(name, "event_cleanup")) assert(event_object.n_waiters == 0);
    if (!strcmp(name, "finish_pending")) assert(u(a) == 0);
    if (!strcmp(name, "callback_nba")) assert(u(a) == 1);
    if (!strcmp(name, "fork_self_disable") || !strcmp(name, "fork_ancestor_disable") ||
        !strcmp(name, "reactive_fixed_point") || !strcmp(name, "pre_nba_reentry") ||
        !strcmp(name, "preponed_once")) assert(observed == 1);
    return 0;
}
