/* Regressions at the public generated-model/runtime boundary. */
#ifdef NDEBUG
#undef NDEBUG
#endif
#define LLG_MODEL_PROCESS_ABI 3
#include "llg_rt.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    llg_co_frame_t co;
    llg_fork_group_t* group;
} probe_frame_t;
LLG_CO_ROOT_FRAME_OK(probe_frame_t);
#define PROBE_PROCESS(name, waits)                                         \
    static llg_co_status_t name(llg_co_frame_t*, llg_co_chain_t*);         \
    static const llg_co_site_t name##_sites[(waits) + 1] = {{0}};          \
    static const llg_co_desc_t name##_desc = {                             \
        name, #name, sizeof(probe_frame_t), name##_sites, (waits) + 1, 0}; \
    static llg_co_status_t name(llg_co_frame_t* co, llg_co_chain_t* ch)
#define PROBE_BEGIN_0()                                                    \
    probe_frame_t* F = (probe_frame_t*)co;                                 \
    llg_proc_t* self = LLG_CO_OWNER(ch, llg_proc_t);                       \
    (void)F;                                                               \
    (void)self;                                                            \
    LLG_CO_DISPATCH_BEGIN(co) LLG_CO_DISPATCH_END(co)
#define PROBE_BEGIN_1()                                                    \
    probe_frame_t* F = (probe_frame_t*)co;                                 \
    llg_proc_t* self = LLG_CO_OWNER(ch, llg_proc_t);                       \
    (void)F;                                                               \
    (void)self;                                                            \
    LLG_CO_DISPATCH_BEGIN(co) LLG_CO_RESUME_CASE(1) LLG_CO_DISPATCH_END(co)
#define PROBE_BEGIN_2()                                                    \
    probe_frame_t* F = (probe_frame_t*)co;                                 \
    llg_proc_t* self = LLG_CO_OWNER(ch, llg_proc_t);                       \
    (void)F;                                                               \
    (void)self;                                                            \
    LLG_CO_DISPATCH_BEGIN(co) LLG_CO_RESUME_CASE(1)                        \
        LLG_CO_RESUME_CASE(2) LLG_CO_DISPATCH_END(co)
#define PROBE_AWAIT(site, arm) LLG_CO_AWAIT(co, ch, site, arm)
#define PROBE_DONE() return LLG_CO_DONE

static sv4_t a, b, source;
static sv4_t zero4, zero8, one8, five8, twenty_five8;
static double real_target;
static llg_net_t net;
static sv4_t* net_drivers[1];
static const uint8_t net_strength0[1] = {6};
static const uint8_t net_strength1[1] = {6};
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
PROBE_PROCESS(forced_real_nba, 2) {
    PROBE_BEGIN_2();
    llg_force_real(&real_target, eval_real, NULL, 0);
    llg_nba_d(&real_target, 9.0);
    llg_nba_d_after(&real_target, 10.0, 1);
    PROBE_AWAIT(1, llg_arm_time(self, 2));
    assert(real_target == 3.5);
    llg_release_real(&real_target);
    assert(real_target == 3.5);
    llg_nba_d(&real_target, 7.0);
    PROBE_AWAIT(2, llg_arm_time(self, 1));
    assert(real_target == 7.0);
    PROBE_DONE();
}
PROBE_PROCESS(force_overlap, 0) {
    PROBE_BEGIN_0();
    llg_force_part_t whole = part(&net.resolved, &net, 7, 0);
    llg_force_part_t low = part(&net.resolved, &net, 3, 0);
    llg_force_expr_parts(&whole, 1, 0, 0, eval_ff, NULL, 0);
    llg_force_expr_parts(&low, 1, 0, 0, eval_zero, NULL, 0);
    assert(u(net.resolved) == 240);
    llg_net_write(&net, 0, twenty_five8);
    llg_release_parts(&low, 1, 0, 0);
    assert(u(net.resolved) == 245);
    llg_release_parts(&whole, 1, 0, 0);
    assert(u(net.resolved) == 0x25);
    PROBE_DONE();
}
PROBE_PROCESS(force_partial_release, 0) {
    PROBE_BEGIN_0();
    llg_force_part_t whole = part(&net.resolved, &net, 7, 0);
    llg_force_part_t low = part(&net.resolved, &net, 3, 0);
    llg_force_expr_parts(&whole, 1, 0, 0, eval_ff, NULL, 0);
    llg_release_parts(&low, 1, 0, 0);
    assert(u(net.resolved) == 240);
    llg_net_write(&net, 0, five8);
    assert(u(net.resolved) == 245);
    llg_release_parts(&whole, 1, 0, 0);
    assert(u(net.resolved) == 5);
    PROBE_DONE();
}
PROBE_PROCESS(force_concat_release, 0) {
    PROBE_BEGIN_0();
    sv4_replace(&a, sv4_from_u64(0, 4, 0));
    sv4_replace(&b, sv4_from_u64(0, 4, 0));
    llg_force_part_t parts[2] = {part(&a, NULL, 3, 0), part(&b, NULL, 3, 0)};
    parts[0].value_lsb = 4;
    llg_force_expr_parts(parts, 2, 0, 0, eval_ff, NULL, 0);
    llg_force_part_t single = part(&a, NULL, 3, 0);
    llg_release_parts(&single, 1, 0, 0);
    llg_ba(&a, zero4);
    llg_ba(&b, zero4);
    assert(u(a) == 0 && u(b) == 15);
    PROBE_DONE();
}
PROBE_PROCESS(force_slot_reuse, 0) {
    PROBE_BEGIN_0();
    llg_force_part_t low = part(&net.resolved, &net, 3, 0);
    llg_force_part_t high = part(&net.resolved, &net, 7, 4);
    llg_force_part_t whole = part(&net.resolved, &net, 7, 0);
    llg_force_expr_parts(&low, 1, 0, 0, eval_zero, NULL, 0);
    llg_force_expr_parts(&high, 1, 0, 0, eval_zero, NULL, 0);
    llg_release_parts(&low, 1, 0, 0);
    llg_force_expr_parts(&whole, 1, 0, 0, eval_ff, NULL, 0);
    assert(u(net.resolved) == 255);
    PROBE_DONE();
}
PROBE_PROCESS(wait_event, 1) {
    PROBE_BEGIN_1();
    PROBE_AWAIT(1, llg_arm_event(self, &event_handle));
    observed++;
    PROBE_DONE();
}
PROBE_PROCESS(finish_later, 1) {
    PROBE_BEGIN_1();
    PROBE_AWAIT(1, llg_arm_time(self, 1));
    llg_rt_finish();
    return LLG_CO_EXIT;
}
PROBE_PROCESS(self_disabling_child, 0) {
    PROBE_BEGIN_0();
    llg_disable_target(self, 1, 1);
    LLG_CO_EXIT_CHECK(ch);
    assert(!"disabled branch returned");
    abort();
}
PROBE_PROCESS(nested_child, 1) {
    PROBE_BEGIN_1();
    F->group = llg_fork_group_new(LLG_JOIN);
    llg_fork(&self_disabling_child_desc, "nested disabling child", F->group);
    PROBE_AWAIT(1, llg_arm_join(self, F->group));
    assert(!"disabled ancestor returned");
    PROBE_DONE();
}
static int nested;
PROBE_PROCESS(fork_parent, 1) {
    PROBE_BEGIN_1();
    F->group = llg_fork_group_new_target(LLG_JOIN, 1, 1);
    llg_fork(nested ? &nested_child_desc : &self_disabling_child_desc,
             "disabling child", F->group);
    PROBE_AWAIT(1, llg_arm_join(self, F->group));
    observed++;
    PROBE_DONE();
}
PROBE_PROCESS(assert_before_design, 1) {
    PROBE_BEGIN_1();
    PROBE_AWAIT(1, llg_arm_any(self, (sv4_t*[]){&b}, 1));
    assert(u(a) == 0);
    observed++;
    PROBE_DONE();
}
static void active_update(void* data) { (void)data; llg_ba(&a, one8); }
PROBE_PROCESS(reactive_writer, 0) {
    PROBE_BEGIN_0();
    llg_schedule_region_callback(LLG_REGION_ACTIVE, active_update, NULL);
    llg_nba(&b, one8);
    PROBE_DONE();
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
    sv4_t sample = SV4_EMPTY;
    assert(llg_sampled_copy(&a, &sample));
    assert(u(sample) == 0);
    sv4_destroy(&sample);
    observed++;
}
static void before_postponed(void* data) {
    (void)data;
    llg_ba(&a, one8);
    llg_schedule_region_callback(LLG_REGION_ACTIVE, sample_observer, NULL);
}
PROBE_PROCESS(nba_then_done, 0) {
    PROBE_BEGIN_0();
    llg_nba(&a, one8);
    PROBE_DONE();
}
PROBE_PROCESS(finish_now, 0) {
    PROBE_BEGIN_0();
    llg_rt_finish();
    return LLG_CO_EXIT;
}
static void monitor_eval(sv4_t* out, void* context) {
    (void)context;
    sv4_copy(&out[0], &a);
}
PROBE_PROCESS(monitor_reenable, 2) {
    PROBE_BEGIN_2();
    llg_monitor_with_reads("value=%0d", 1, monitor_eval, (sv4_t*[]){&a}, 1);
    PROBE_AWAIT(1, llg_arm_time(self, 1));
    llg_monitor_set(1);
    PROBE_AWAIT(2, llg_arm_time(self, 1));
    PROBE_DONE();
}
static void event_alias_readonly(void* data) { (void)data; llg_event_assign_null(&event_handle); }
PROBE_PROCESS(illegal_early_spawn, 0) {
    PROBE_BEGIN_0();
    llg_spawn_in_region(&wait_event_desc, "invalid preponed", LLG_REGION_PREPONED);
    PROBE_DONE();
}
static void callback_nba(void* data) { (void)data; llg_nba(&a, one8); }
static void init(void) {
    llg_rt_init();
    zero4 = sv4_from_u64(0, 4, 0);
    zero8 = v(0);
    one8 = v(1);
    five8 = v(5);
    twenty_five8 = v(0x25);
    a = sv4_clone(&zero8);
    b = sv4_clone(&zero8);
    source = sv4_clone(&zero8);
    real_target = 0;
    observed = 0;
    memset(&net, 0, sizeof(net));
    net.width = 8;
    net.resolved = sv4_clone(&zero8);
    net.n_drivers = 1;
    net_drivers[0] = &source;
    net.drivers = net_drivers;
    net.strength0 = net_strength0;
    net.strength1 = net_strength1;
    memset(&event_object, 0, sizeof(event_object));
    event_handle.object = &event_object;
}
int main(int argc, char** argv) {
    assert(argc == 2);
    init();
    const char* name = argv[1];
    if (!strcmp(name, "forced_real_nba")) llg_spawn(&forced_real_nba_desc, name);
    else if (!strcmp(name, "force_overlap")) llg_spawn(&force_overlap_desc, name);
    else if (!strcmp(name, "force_partial_release")) llg_spawn(&force_partial_release_desc, name);
    else if (!strcmp(name, "force_concat_release")) llg_spawn(&force_concat_release_desc, name);
    else if (!strcmp(name, "force_slot_reuse")) llg_spawn(&force_slot_reuse_desc, name);
    else if (!strcmp(name, "event_cleanup")) {
        llg_spawn(&wait_event_desc, name);
        llg_spawn(&finish_later_desc, "finish");
    } else if (!strcmp(name, "fork_self_disable") || !strcmp(name, "fork_ancestor_disable")) {
        nested = !strcmp(name, "fork_ancestor_disable");
        llg_spawn(&fork_parent_desc, name);
    } else if (!strcmp(name, "reactive_fixed_point")) {
        llg_spawn_in_region(&assert_before_design_desc, name, LLG_REGION_REACTIVE);
        llg_spawn_in_region(&reactive_writer_desc, "reactive writer", LLG_REGION_REACTIVE);
    } else if (!strcmp(name, "pre_nba_reentry")) {
        llg_schedule_region_callback(LLG_REGION_PRE_NBA, pre_nba_callback, NULL);
        llg_schedule_region_callback(LLG_REGION_NBA, nba_value_observer, NULL);
    } else if (!strcmp(name, "preponed_once")) {
        llg_sampled_register(&a);
        llg_schedule_region_callback(LLG_REGION_PRE_POSTPONED, before_postponed, NULL);
    } else if (!strcmp(name, "finish_pending")) {
        llg_spawn(&nba_then_done_desc, "NBA producer");
        llg_spawn(&finish_now_desc, "finish");
    } else if (!strcmp(name, "monitor_reenable")) llg_spawn(&monitor_reenable_desc, name);
    else if (!strcmp(name, "event_readonly"))
        llg_schedule_region_callback(LLG_REGION_OBSERVED, event_alias_readonly, NULL);
    else if (!strcmp(name, "early_spawn")) llg_spawn(&illegal_early_spawn_desc, name);
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
    llg_event_object_reset(&event_object);
    llg_rt_cleanup();
    sv4_destroy(&a);
    sv4_destroy(&b);
    sv4_destroy(&source);
    sv4_destroy(&net.resolved);
    sv4_destroy(&zero4);
    sv4_destroy(&zero8);
    sv4_destroy(&one8);
    sv4_destroy(&five8);
    sv4_destroy(&twenty_five8);
    return 0;
}
