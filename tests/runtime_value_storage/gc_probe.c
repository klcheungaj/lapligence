/* SIM-018 collector contract on the production runtime, without generated
 * code: unreachable cycles are reclaimed while every root kind (registered
 * producers, pins, interior addresses, handle cells, frame maps of suspended
 * processes) keeps its graph; an allocation failure at any collector
 * allocation abandons the collection with every object intact; repeated
 * collection keeps memory bounded across an epoch wrap; teardown finalizes
 * everything. Collector allocations go through the injectable wrappers below
 * (the facade is compiled into this unit). */
#define _GNU_SOURCE
#include "llg_rt.h"
#include "llg_container.h"
#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <math.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static long gc_probe_calls;
static long gc_probe_fail_at = -1;

static int gc_probe_inject(void) {
    if (gc_probe_fail_at < 0) return 0;
    return gc_probe_calls++ == gc_probe_fail_at;
}
static void* gc_probe_malloc(size_t size) {
    return gc_probe_inject() ? NULL : malloc(size);
}
static void* gc_probe_calloc(size_t count, size_t size) {
    return gc_probe_inject() ? NULL : calloc(count, size);
}
static void* gc_probe_realloc(void* pointer, size_t size) {
    return gc_probe_inject() ? NULL : realloc(pointer, size);
}
#define malloc gc_probe_malloc
#define calloc gc_probe_calloc
#define realloc gc_probe_realloc
#include "llg_rt.c"
#undef malloc
#undef calloc
#undef realloc
#include "probe.h"
#include "probe_co.h"

enum { NODE_LIVE = 0x4c495645u, NODE_DEAD = 0x44454144u, NODE_PAYLOAD = 4 };

typedef struct gc_node {
    llg_gc_header_t gc;
    struct gc_node* edges[2];
    struct gc_node** fan;
    size_t fan_count;
    uint32_t id;
    uint32_t magic;
    sv4_t value;
    int payload[NODE_PAYLOAD];
} gc_node;

static uint64_t finalized;
static uint64_t condemned_seen;

static void node_trace(void* object, llg_gc_tracer_t* tracer) {
    gc_node* node = (gc_node*)object;
    llg_gc_visit(tracer, node->edges[0]);
    llg_gc_visit(tracer, node->edges[1]);
    for (size_t i = 0; i < node->fan_count; ++i) llg_gc_visit(tracer, node->fan[i]);
}

static int node_interior(const void* object, const llg_gc_tracer_t* tracer) {
    const gc_node* node = (const gc_node*)object;
    for (int i = 0; i < NODE_PAYLOAD; ++i)
        if (llg_gc_interior_hit(tracer, &node->payload[i])) return 1;
    return 0;
}

static void node_finalize(void* object) {
    gc_node* node = (gc_node*)object;
    sv4_destroy(&node->value);
    free(node->fan);
    ++finalized;
}

static void node_condemn(void* object) {
    ((gc_node*)object)->magic = NODE_DEAD;
    ++condemned_seen;
}

static const llg_gc_type_t node_type = {
    "probe node", node_trace, node_interior, node_finalize, node_condemn};

static uint32_t next_id;

static gc_node* node_new(void) {
    gc_node* node = (gc_node*)llg_gc_alloc(sizeof(gc_node), &node_type);
    node->id = ++next_id;
    node->magic = NODE_LIVE;
    node->value = sv4_from_u64(node->id, 65, 0);
    for (int i = 0; i < NODE_PAYLOAD; ++i) node->payload[i] = (int)node->id + i;
    return node;
}

static gc_node* cycle_new(void) {
    gc_node* a = node_new();
    gc_node* b = node_new();
    a->edges[0] = b;
    b->edges[0] = a;
    return a;
}

static void check_node(const gc_node* node) {
    CHECK(node->magic == NODE_LIVE);
    CHECK(node->payload[NODE_PAYLOAD - 1] == (int)node->id + NODE_PAYLOAD - 1);
    sv4_t copy = sv4_clone(&node->value);
    expect_number(copy, node->id);
}

enum { ROOTS = 4, INTERIOR_ROOTS = 128 };
static gc_node* roots[ROOTS];
static const void* interior_roots[INTERIOR_ROOTS];

static void probe_roots(llg_gc_tracer_t* tracer, void* context) {
    CHECK(context == (void*)roots);
    for (int i = 0; i < ROOTS; ++i) llg_gc_visit(tracer, roots[i]);
    for (int i = 0; i < INTERIOR_ROOTS; ++i) llg_gc_visit_interior(tracer, interior_roots[i]);
}

static llg_gc_stats_t stats(void) {
    llg_gc_stats_t result;
    llg_gc_get_stats(&result);
    return result;
}

// Fixed policy regardless of the caller's environment.
static void start(int verify) {
    llg_rt_init();
    llg_gc.disabled = 0;
    llg_gc.stress = 0;
    llg_gc.verify = verify;
    llg_gc.print_stats = 0;
    llg_gc.base_threshold = llg_gc.threshold = 1000000;
    llg_gc.growth_percent = 100;
    memset(roots, 0, sizeof(roots));
    // Assigned, not memset: MSVC reports C4090 (const qualifiers) for an
    // array of pointers to const passed as void*, and /WX makes it an error.
    for (int i = 0; i < INTERIOR_ROOTS; ++i) interior_roots[i] = NULL;
    finalized = condemned_seen = 0;
    CHECK(llg_gc_register_roots(probe_roots, (void*)roots));
}

static void finish(void) {
    llg_gc_stats_t before = stats();
    llg_rt_cleanup();
    llg_gc_teardown();
    // Teardown finalizes every live and condemned object exactly once.
    CHECK(finalized == before.freed + before.live + before.condemned);
    CHECK(finalized == before.allocated);
    CHECK(stats().allocated == 0 && stats().live == 0);
    CHECK(llg_gc.capacity == 0 && !llg_gc.index);
    CHECK(value_test_live() == 0);
}

static void cycles_are_reclaimed_and_rooted_graphs_survive(void) {
    start(0);
    gc_node* head = node_new();
    roots[0] = head;
    for (int i = 1; i < 100; ++i) {
        gc_node* node = node_new();
        node->edges[0] = head;
        head->edges[1] = node;
        head = node;
    }
    for (int i = 0; i < 1000; ++i) (void)cycle_new();
    gc_node* self = node_new();
    self->edges[0] = self;
    CHECK(stats().live == 2101);
    llg_gc_collect();
    llg_gc_stats_t after = stats();
    CHECK(after.collections == 1 && after.failed_collections == 0);
    CHECK(after.freed == 2001 && after.live == 100 && after.peak_live == 2101);
    CHECK(finalized == 2001);
    size_t walked = 0;
    for (const gc_node* node = roots[0]; node; node = node->edges[1], ++walked) {
        check_node(node);
        CHECK(llg_gc_is_object(node));
    }
    CHECK(walked == 100);
    // A collection with nothing new keeps the same graph.
    llg_gc_collect();
    CHECK(stats().freed == 2001 && stats().live == 100);
    roots[0] = NULL;
    llg_gc_collect();
    CHECK(stats().live == 0 && stats().freed == 2101);
    finish();
}

static void pins_retain_until_the_last_release(void) {
    start(0);
    gc_node* a = cycle_new();
    int local = 0;
    CHECK(!llg_gc_pin(&local) && !llg_gc_pin(NULL) && !llg_gc_unpin(&local));
    CHECK(llg_gc_pin(a) && llg_gc_pin(a));
    CHECK(stats().pinned == 1);
    llg_gc_collect();
    CHECK(stats().live == 2);
    check_node(a);
    check_node(a->edges[0]);
    CHECK(llg_gc_unpin(a));
    llg_gc_collect();
    CHECK(stats().live == 2 && stats().pinned == 1);
    CHECK(llg_gc_unpin(a));
    CHECK(stats().pinned == 0);
    llg_gc_collect();
    CHECK(stats().live == 0 && stats().freed == 2);
    finish();
}

static void interior_addresses_and_handle_cells_are_roots(void) {
    start(0);
    gc_node* x = cycle_new();
    gc_node* y = x->edges[0];
    interior_roots[0] = &y->payload[2];
    llg_gc_collect();
    CHECK(stats().live == 2);
    check_node(x);
    check_node(y);
    interior_roots[0] = NULL;
    llg_gc_collect();
    CHECK(stats().live == 0);

    gc_node* held = cycle_new();
    llg_value_scope_t* scope =
        llg_value_scope_begin_object(sizeof(void*), llg_gc_handle_cell_drop);
    *(void**)llg_value_scope_object(scope) = held;
    (void)cycle_new();
    llg_gc_collect();
    CHECK(stats().live == 2);
    check_node(held);
    check_node(held->edges[0]);
    llg_value_scope_end(scope);
    llg_gc_collect();
    CHECK(stats().live == 0 && stats().freed == 6);
    finish();
}

typedef struct {
    llg_co_frame_t co;
    gc_node* held;
} holder_frame_t;
LLG_CO_ROOT_FRAME_OK(holder_frame_t);

static unsigned mapped_checked;
static uint32_t unmapped_magic;

LLG_PROBE_PROCESS(holder_mapped, holder_frame_t, 1) {
    LLG_PROBE_BEGIN(holder_frame_t, 1);
    F->held = cycle_new();
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 10));
    check_node(F->held);
    check_node(F->held->edges[0]);
    ++mapped_checked;
    LLG_PROBE_DONE();
}

// Same body, no registered frame map: its frame is not a root.
LLG_PROBE_PROCESS(holder_unmapped, holder_frame_t, 1) {
    LLG_PROBE_BEGIN(holder_frame_t, 1);
    F->held = cycle_new();
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 10));
    unmapped_magic = F->held->magic;
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(churner, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    for (F->index = 0; F->index < 20; F->index++) {
        for (int i = 0; i < 10; ++i) (void)cycle_new();
        LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    }
    LLG_PROBE_DONE();
}

static const llg_gc_frame_slot_t holder_slots[] = {
    {(uint32_t)offsetof(holder_frame_t, held), LLG_GC_FRAME_HANDLE}};
static const uint32_t holder_first[] = {0, 0};
static const uint32_t holder_count[] = {0, 1};
static const llg_gc_frame_map_t holder_map = {holder_slots, holder_first,
                                              holder_count, 2};

static void suspended_frames_are_traced_at_safe_points(void) {
    start(1);
    llg_gc.stress = 1;
    mapped_checked = 0;
    unmapped_magic = 0;
    CHECK(llg_gc_register_frame_map(&holder_mapped_desc, &holder_map));
    CHECK(llg_spawn(&holder_mapped_desc, "mapped"));
    CHECK(llg_spawn(&holder_unmapped_desc, "unmapped"));
    CHECK(llg_spawn(&churner_desc, "churner"));
    llg_rt_run();
    CHECK(mapped_checked == 1);
    CHECK(unmapped_magic == NODE_DEAD);
    llg_gc_stats_t after = stats();
    CHECK(after.collections >= 20 && after.freed == 0);
    // The churn and the unmapped holder's cycle, then the mapped holder's
    // cycle once its process completed.
    CHECK(after.condemned == condemned_seen && after.condemned >= 402);
    CHECK(after.condemned + after.live == after.allocated);
    finish();
}

enum { FAN = 600, GARBAGE = 600 };

// Rooted fan-out wider than the initial mark stack, many interior roots
// (growing the interior set) and unreachable cycles.
static gc_node* failure_scenario(void) {
    gc_node* hub = node_new();
    hub->fan = (gc_node**)malloc(FAN * sizeof(*hub->fan));
    CHECK(hub->fan);
    hub->fan_count = FAN;
    for (int i = 0; i < FAN; ++i) {
        hub->fan[i] = node_new();
        hub->fan[i]->edges[0] = hub;
    }
    roots[0] = hub;
    for (int i = 0; i < INTERIOR_ROOTS; ++i) {
        gc_node* held = cycle_new();
        interior_roots[i] = &held->payload[i % NODE_PAYLOAD];
    }
    for (int i = 0; i < GARBAGE / 2; ++i) (void)cycle_new();
    return hub;
}

static void check_scenario(const gc_node* hub) {
    check_node(hub);
    for (int i = 0; i < FAN; ++i) {
        check_node(hub->fan[i]);
        CHECK(hub->fan[i]->edges[0] == hub);
    }
    for (int i = 0; i < INTERIOR_ROOTS; ++i) {
        const gc_node* held = (const gc_node*)((const char*)interior_roots[i] -
            offsetof(gc_node, payload) - (size_t)(i % NODE_PAYLOAD) * sizeof(int));
        check_node(held);
        check_node(held->edges[0]);
    }
}

static void allocation_failure_abandons_collection(int verify) {
    const uint64_t kept = 1 + FAN + 2 * INTERIOR_ROOTS;
    unsigned abandoned = 0, compaction_failures = 0;
    for (long fail_at = 0;; ++fail_at) {
        start(verify);
        gc_node* hub = failure_scenario();
        llg_gc_stats_t before = stats();
        uint64_t threshold = llg_gc.threshold;
        gc_probe_calls = 0;
        gc_probe_fail_at = fail_at;
        llg_gc_collect();
        int injected = gc_probe_calls > fail_at;
        gc_probe_fail_at = -1;
        llg_gc_stats_t after = stats();
        check_scenario(hub);
        if (!injected) {
            CHECK(after.collections == 1 && after.failed_collections == 0);
            CHECK(after.live == kept);
            CHECK((verify ? after.condemned : after.freed) == GARBAGE);
            roots[0] = NULL;
            finish();
            break;
        }
        if (after.failed_collections == 1) {
            // Nothing was finalized or condemned and the retry is deferred.
            ++abandoned;
            CHECK(after.collections == 0 && after.live == before.live);
            CHECK(after.freed == 0 && after.condemned == 0 && finalized == 0);
            CHECK(condemned_seen == 0);
            CHECK(llg_gc.threshold == threshold * 2 && !llg_gc_pending);
            CHECK(!llg_gc.collecting);
            // The next collection succeeds and reclaims the garbage.
            llg_gc_collect();
            CHECK(stats().collections == 1 && stats().live == kept);
        } else {
            // Index compaction after a committed sweep is optional.
            ++compaction_failures;
            CHECK(after.collections == 1 && after.live == kept);
        }
        CHECK((verify ? stats().condemned : stats().freed) == GARBAGE);
        check_scenario(hub);
        finish();
        CHECK(fail_at < 64);
    }
    // Mark stack, interior set (first allocation and growth), dead list and,
    // under verify, the condemned list; then the index compaction.
    CHECK(abandoned >= (verify ? 5u : 4u));
    CHECK(compaction_failures >= 1);
}

static void repeated_collection_stays_bounded(void) {
    start(0);
    roots[0] = cycle_new();
    roots[1] = cycle_new();
    // Cross an epoch wrap: stale marks must not keep or lose objects.
    llg_gc.epoch = UINT32_MAX - 5;
    size_t max_capacity = 0;
    for (int round = 0; round < 300; ++round) {
        for (int i = 0; i < 500; ++i) (void)cycle_new();
        if (round % 7 == 0) roots[2] = cycle_new();
        llg_gc_collect();
        CHECK(stats().live == 6);
        if (llg_gc.capacity > max_capacity) max_capacity = llg_gc.capacity;
        check_node(roots[0]);
        check_node(roots[1]->edges[0]);
        check_node(roots[2]);
    }
    CHECK(llg_gc.epoch < 300);
    CHECK(stats().collections == 300 && stats().failed_collections == 0);
    CHECK(stats().freed == 300u * 1000u + 2u * 42u);
    CHECK(max_capacity <= 2048);
    // A replaced roots[2] cycle is allocated before the old one is freed.
    CHECK(stats().peak_live == 1008);
    finish();
}

static void allocation_policy_requests_safe_points(void) {
    start(0);
    llg_gc.base_threshold = llg_gc.threshold = 100;
    llg_gc_pending = 0;
    for (int i = 0; i < 99; ++i) roots[0] = node_new();
    CHECK(!llg_gc_pending);
    (void)node_new();
    CHECK(llg_gc_pending);
    // Keep 300: the next threshold grows to live * 100%.
    for (int i = 0; i < 300; ++i) {
        gc_node* node = node_new();
        node->edges[0] = roots[1];
        roots[1] = node;
    }
    llg_gc_collect();
    CHECK(!llg_gc_pending && stats().live == 301 && llg_gc.threshold == 301);
    llg_gc.disabled = 1;
    for (int i = 0; i < 400; ++i) (void)node_new();
    CHECK(!llg_gc_pending);
    llg_gc_collect();
    CHECK(stats().collections == 1 && stats().live == 701);
    finish();
}

int main(void) {
    value_test_reset_stats();
    cycles_are_reclaimed_and_rooted_graphs_survive();
    pins_retain_until_the_last_release();
    interior_addresses_and_handle_cells_are_roots();
    suspended_frames_are_traced_at_safe_points();
    allocation_failure_abandons_collection(0);
    allocation_failure_abandons_collection(1);
    repeated_collection_stays_bounded();
    allocation_policy_requests_safe_points();
    puts("gc probe ok");
    return 0;
}
