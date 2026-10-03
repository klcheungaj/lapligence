#define _GNU_SOURCE
#include "llg_rt.h"
#include "llg_container.h"
#include "probe.h"
#include <string.h>
#include <stdarg.h>
#include <math.h>
#include <limits.h>
#include <ctype.h>
#include <errno.h>
#if defined(__unix__) || defined(__APPLE__)
#include <sys/resource.h>
#endif

static size_t scheduler_allocations;
static void* counted_malloc(size_t bytes) {
    scheduler_allocations++;
    return malloc(bytes);
}
static void* counted_calloc(size_t count, size_t bytes) {
    scheduler_allocations++;
    return calloc(count, bytes);
}
static void* counted_realloc(void* pointer, size_t bytes) {
    scheduler_allocations++;
    return realloc(pointer, bytes);
}
#define malloc counted_malloc
#define calloc counted_calloc
#define realloc counted_realloc
#include "llg_rt.c"
#undef malloc
#undef calloc
#undef realloc

static llg_proc_t* test_proc(void) {
    llg_proc_t* proc = calloc(1, sizeof(*proc));
    CHECK(proc != NULL);
    register_proc(proc);
    proc->handle = process_handle_new(proc);
    proc->region = LLG_REGION_ACTIVE;
    return proc;
}

static void ready_again(llg_proc_t* proc) {
    llg_region_t region = proc->wait.resume_region;
    wake_proc(proc);
    CHECK(dequeue_region(region) == proc);
    CHECK(proc->wait.kind == W_NONE && !proc->wait.subscriptions);
}

static void single_and_heap_paths(void) {
    sv4_t narrow = sv4_from_masks(0x1234, 0x40, 0x80, 64, 1);
    sv4_t wide = sv4_zero(65, 0);
    sv4_t empty = SV4_EMPTY;
    llg_event_object_t first_object = {0};
    llg_event_t first = {&first_object};
    llg_event_object_t second_object = {0};
    llg_event_t second = {&second_object};
    llg_rt_init();
    llg_proc_t* keeper = test_proc();
    llg_proc_t* proc = test_proc();
    sv4_t* signals[] = {&narrow, &wide};
    g.current = keeper;
    CHECK(llg_arm_any(keeper, signals, 2) == LLG_CO_ARM_SUSPEND);
    g.current = proc;
    for (int cycle = 0; cycle < 100; cycle++) {
        size_t allocations = scheduler_allocations;
        size_t value_allocations = value_test_allocations();
        llg_event_spec_t spec = {&narrow, LLG_EV_POSEDGE};
        CHECK(llg_arm_any_events(proc, &spec, 1) == LLG_CO_ARM_SUSPEND);
        spec.sig = &wide;
        spec.kind = LLG_EV_NEGEDGE;
        CHECK(proc->wait.kind == W_EVENTS_INLINE);
        CHECK(proc->wait.subscriptions == &proc->wait.payload.single.subscription);
        CHECK(proc->wait.payload.single.specs[0].sig == &narrow);
        CHECK(proc->wait.payload.single.specs[0].kind == LLG_EV_POSEDGE);
        CHECK(wait_inline_same(&proc->wait, narrow));
        CHECK(scheduler_allocations == allocations);
        CHECK(value_test_allocations() == value_allocations);
        ready_again(proc);
        CHECK(llg_arm_any(proc, signals, 2) == LLG_CO_ARM_SUSPEND);
        CHECK(proc->wait.kind == W_EVENTS);
        CHECK(proc->wait.payload.expression.last[1].width == 65);
        ready_again(proc);
        CHECK(llg_arm_edge(proc, &wide, 0) == LLG_CO_ARM_SUSPEND);
        CHECK(proc->wait.kind == W_EVENTS);
        ready_again(proc);
        allocations = scheduler_allocations;
        value_allocations = value_test_allocations();
        CHECK(llg_arm_any(proc, signals, 1) == LLG_CO_ARM_SUSPEND);
        CHECK(proc->wait.kind == W_EVENTS_INLINE);
        CHECK(scheduler_allocations == allocations);
        CHECK(value_test_allocations() == value_allocations);
        ready_again(proc);
        CHECK(llg_arm_level(proc, &wide, wide) == LLG_CO_ARM_SUSPEND);
        CHECK(proc->wait.kind == W_LEVEL);
        ready_again(proc);
        allocations = scheduler_allocations;
        value_allocations = value_test_allocations();
        CHECK(llg_arm_level(proc, &narrow, narrow) == LLG_CO_ARM_SUSPEND);
        CHECK(proc->wait.kind == W_LEVEL_INLINE);
        CHECK(scheduler_allocations == allocations);
        CHECK(value_test_allocations() == value_allocations);
        ready_again(proc);
        const llg_event_t* events[] = {&first, &second};
        CHECK(llg_arm_events(proc, events, 2) == LLG_CO_ARM_SUSPEND);
        CHECK(proc->wait.payload.event.evs != &proc->wait.payload.event.inline_ev);
        ready_again(proc);
        allocations = scheduler_allocations;
        CHECK(llg_arm_events(proc, events, 1) == LLG_CO_ARM_SUSPEND);
        events[0] = &second;
        CHECK(proc->wait.payload.event.evs == &proc->wait.payload.event.inline_ev);
        CHECK(proc->wait.payload.event.evs[0] == first.object);
        CHECK(scheduler_allocations == allocations);
        llg_event_trigger(&first);
        CHECK(dequeue_region(LLG_REGION_ACTIVE) == proc);
        CHECK(!proc->wait.subscriptions && proc->wait.kind == W_NONE);
        CHECK(llg_arm_time(proc, cycle % 2) == LLG_CO_ARM_SUSPEND);
        ready_again(proc);
    }
    CHECK(llg_arm_edge(proc, &empty, 1) == LLG_CO_ARM_SUSPEND);
    CHECK(proc->wait.kind == W_EVENTS_INLINE);
    CHECK(proc->wait.payload.single.width == 0);
    ready_again(proc);
    sv4_t padded = sv4_zero(5, 1);
    llg_sv4_set_word(&padded, 0, 1, 2, 4);
    llg_sv4_set_signed(&padded, -1);
    CHECK(llg_arm_level(proc, &narrow, padded) == LLG_CO_ARM_SUSPEND);
    const llg_wait_inline_payload_t* copied = &proc->wait.payload.single;
    CHECK(copied->words[0].aval == 3 && copied->words[0].bval == 6);
    CHECK(copied->is_signed == 1);
    CHECK(llg_sv4_word(padded, 0, LLG_SV4_BITS) == 1 &&
          llg_sv4_word(padded, 0, LLG_SV4_X) == 2 &&
          llg_sv4_word(padded, 0, LLG_SV4_Z) == 4 && llg_sv4_signed(padded) == 1);
    ready_again(proc);
    sv4_destroy(&padded);
    CHECK(llg_arm_event(proc, NULL) == LLG_CO_ARM_SUSPEND);
    CHECK(proc->wait.payload.event.evs[0] == NULL);
    ready_again(proc);
    g.current = NULL;
    llg_rt_cleanup();
    llg_event_object_reset(&first_object);
    llg_event_object_reset(&second_object);
    sv4_destroy(&narrow);
    sv4_destroy(&wide);
    CHECK(value_test_live() == 0);
}

static void state_edges_and_resize(void) {
    sv4_t signal = sv4_zero(1, 0);
    sv4_t states[] = {
        sv4_from_masks(0, 1, 0, 1, 0),
        sv4_from_masks(0, 0, 1, 1, 0),
        sv4_from_u64(1, 1, 0),
        sv4_zero(1, 0),
    };
    llg_rt_init();
    llg_proc_t* proc = test_proc();
    g.current = proc;
    CHECK(llg_arm_edge(proc, &signal, 1) == LLG_CO_ARM_SUSPEND);
    sig_write(&signal, states[0]); // 0 -> X is a posedge
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == proc);
    CHECK(llg_arm_edge(proc, &signal, 1) == LLG_CO_ARM_SUSPEND);
    sig_write(&signal, states[1]); // X -> Z is not a posedge
    CHECK(proc->wait.kind == W_EVENTS_INLINE);
    CHECK(proc->wait.payload.single.words[0].aval == 0 &&
          proc->wait.payload.single.words[0].bval == 1);
    sig_write(&signal, states[2]); // Z -> 1 is a posedge
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == proc);
    CHECK(llg_arm_edge(proc, &signal, 0) == LLG_CO_ARM_SUSPEND);
    sig_write(&signal, states[1]); // 1 -> Z is a negedge
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == proc);
    CHECK(llg_arm_edge(proc, &signal, 0) == LLG_CO_ARM_SUSPEND);
    sig_write(&signal, states[3]); // Z -> 0 is a negedge
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == proc);
    CHECK(llg_arm_level(proc, &signal, states[1]) == LLG_CO_ARM_SUSPEND);
    probe_put_state(&states[1], 0, 2);
    sig_write(&signal, states[0]);
    CHECK(proc->wait.kind == W_LEVEL_INLINE);
    probe_put_state(&states[1], 0, 3);
    sig_write(&signal, states[1]);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == proc);
    sig_write(&signal, states[3]);
    CHECK(llg_arm_edge(proc, &signal, 1) == LLG_CO_ARM_SUSPEND);
    sv4_t wide = sv4_zero(65, 0);
    sig_write(&signal, wide); // no LSB edge; promote a live inline subscription
    CHECK(proc->wait.kind == W_EVENTS && proc->wait.n_subscriptions == 1);
    CHECK(proc->wait.subscriptions->prev_link != NULL);
    CHECK(proc->wait.payload.expression.last[0].width == 65);
    probe_set_bits(&wide, 0, 1);
    sig_write(&signal, wide);
    CHECK(dequeue_region(LLG_REGION_ACTIVE) == proc);
    CHECK(g.n_wait_sources == 0);
    g.current = NULL;
    llg_rt_cleanup();
    sv4_destroy(&signal);
    sv4_destroy(&wide);
    sv4_destroy_array(states, 4);
    CHECK(value_test_live() == 0);
}

static void copied_complex_waits(void) {
    sv4_t signal = sv4_zero(65, 0);
    sv4_t other = sv4_zero(1, 0);
    llg_event_object_t object = {0};
    llg_event_t event = {&object};
    llg_rt_init();
    llg_proc_t* proc = test_proc();
    g.current = proc;
    llg_wait_dependency_t deps[] = {{0}, {0}};
    deps[0].sig = &signal;
    deps[0].width = 65;
    deps[1].sig = &other;
    CHECK(llg_arm_any_dependencies(proc, deps, 2) == LLG_CO_ARM_SUSPEND);
    deps[0].sig = &other;
    CHECK(proc->wait.kind == W_DEPS);
    CHECK(proc->wait.payload.expression.dependencies[0].sig == &signal);
    ready_again(proc);
    CHECK(llg_arm_edge(proc, &other, 1) == LLG_CO_ARM_SUSPEND);
    ready_again(proc);
    llg_expr_event_spec_t expression = {0};
    expression.sig = &signal;
    expression.kind = LLG_EV_ANY;
    deps[0].sig = &signal;
    expression.dependencies = deps;
    expression.n_dependencies = 2;
    CHECK(llg_arm_expressions(proc, &expression, 1) == LLG_CO_ARM_SUSPEND);
    deps[0].sig = &other;
    CHECK(proc->wait.kind == W_EXPR);
    CHECK(proc->wait.payload.expression.expressions[0].dependencies[0].sig == &signal);
    ready_again(proc);
    CHECK(llg_arm_level(proc, &other, other) == LLG_CO_ARM_SUSPEND);
    ready_again(proc);
    llg_wait_src_t sources[] = {{0}, {0}};
    sources[0].sig = &signal;
    sources[0].kind = LLG_EV_ANY;
    sources[1].ev = &event;
    CHECK(llg_arm_mixed(proc, sources, 2) == LLG_CO_ARM_SUSPEND);
    sources[0].sig = &other;
    CHECK(proc->wait.kind == W_MIXED);
    CHECK(proc->wait.payload.rare->mixed.specs[0].sig == &signal);
    ready_again(proc);
    g.current = NULL;
    llg_rt_cleanup();
    llg_event_object_reset(&object);
    sv4_destroy(&signal);
    sv4_destroy(&other);
    CHECK(value_test_live() == 0);
}

static void cancel_and_teardown(void) {
    for (int mode = 0; mode < 12; mode++) {
        sv4_t signal = sv4_zero(mode % 2 ? 65 : 1, 0);
        llg_event_object_t object = {0};
        llg_event_t event = {&object};
        llg_rt_init();
        llg_proc_t* proc = test_proc();
        g.current = proc;
        if (mode < 4) CHECK(llg_arm_edge(proc, &signal, 1) == LLG_CO_ARM_SUSPEND);
        else if (mode < 8) CHECK(llg_arm_level(proc, &signal, signal) == LLG_CO_ARM_SUSPEND);
        else {
            const llg_event_t* events[] = {&event, &event};
            CHECK(llg_arm_events(proc, events, mode % 2 ? 2 : 1) == LLG_CO_ARM_SUSPEND);
        }
        g.current = NULL;
        if (mode % 4 < 2) {
            llg_kill_proc(proc, 0);
            CHECK(g.wait_count == 0 && g.n_wait_sources == 0);
            reap_retired_procs();
        }
        llg_rt_cleanup();
        CHECK(!g.waiters && !g.wait_sources && !g.wait_count);
        llg_event_object_reset(&object);
        sv4_destroy(&signal);
        CHECK(value_test_live() == 0);
    }
}

int main(void) {
    single_and_heap_paths();
    state_edges_and_resize();
    copied_complex_waits();
    cancel_and_teardown();
    printf("inline waits: OK (process=%zu wait=%zu limbs=%u)\n",
           sizeof(llg_proc_t), sizeof(llg_wait_t), LLG_WAIT_INLINE_LIMBS);
    return 0;
}
