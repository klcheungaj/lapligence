#include "llg_rt.c"
#include "probe.h"
#include "probe_co.h"

static unsigned drops;
static unsigned order;
static llg_proc_t* victim;

static void drop_native(void* object) {
    order = order * 10 + *(unsigned*)object;
    ++drops;
}

typedef struct {
    llg_co_frame_t co;
    llg_value_scope_t packed_scope;
    sv4_t packed[2];
    llg_value_scope_t native_scope;
    unsigned native;
} cell_frame_t;
LLG_CO_ROOT_FRAME_OK(cell_frame_t);

LLG_PROBE_PROCESS(cells, cell_frame_t, 1) {
    LLG_PROBE_BEGIN(cell_frame_t, 1);
    F->packed[0] = (sv4_t)SV4_EMPTY;
    F->packed[1] = (sv4_t)SV4_EMPTY;
    llg_value_scope_register(&F->packed_scope, F->packed, 2);
    CHECK(value_scope_index_find(&F->packed[0]) == &F->packed_scope);
    CHECK(value_scope_index_find(&F->packed[1]) == &F->packed_scope);
    sv4_replace(&F->packed[0], sv4_from_u64(19, 65, 0));
    llg_ba(&F->packed[1], F->packed[0]);
    CHECK(F->packed[1].bits[0] == 19);
    CHECK(find_clocking_edge(&F->packed[1]) != NULL);
    F->native = 2;
    llg_value_scope_register_object(&F->native_scope, &F->native, drop_native);
    CHECK(value_scope_index_find(&F->native) == &F->native_scope);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 1));
    CHECK(F->packed[0].bits[0] == 19);
    llg_value_scope_end(&F->native_scope);
    llg_value_scope_end(&F->packed_scope);
    CHECK(value_scope_index_find(&F->packed[0]) == NULL);
    CHECK(value_scope_index_find(&F->packed[1]) == NULL);
    CHECK(value_scope_index_find(&F->native) == NULL);
    CHECK(F->packed[0].bits == NULL && F->packed[1].bits == NULL);
    CHECK(!F->packed_scope.active && !F->native_scope.active);
    CHECK(find_clocking_edge(&F->packed[1]) == NULL);
    LLG_PROBE_DONE();
}

typedef struct {
    llg_co_frame_t co;
    cell_frame_t child;
    int iteration;
    llg_value_scope_t outer_scope;
    unsigned outer;
} repeat_frame_t;
LLG_CO_ROOT_FRAME_OK(repeat_frame_t);

LLG_PROBE_PROCESS(repeat, repeat_frame_t, 1) {
    LLG_PROBE_BEGIN(repeat_frame_t, 1);
    (void)&cells_desc;
    F->outer = 1;
    llg_value_scope_register_object(&F->outer_scope, &F->outer, drop_native);
    for (F->iteration = 0; F->iteration < 16; ++F->iteration) {
        LLG_CO_CALL(co, ch, 1, cells, &F->child.co);
        CHECK(self->value_scopes == &F->outer_scope);
        CHECK(value_scope_count == 1);
    }
    llg_value_scope_end(&F->outer_scope);
    LLG_PROBE_DONE();
}

LLG_PROBE_SIMPLE_PROCESS(killer, 1) {
    LLG_PROBE_SIMPLE_BEGIN(1);
    LLG_PROBE_AWAIT(1, llg_arm_time(self, 0));
    llg_kill_proc_tree(victim);
    victim = NULL;
    LLG_PROBE_DONE();
}

LLG_PROBE_PROCESS(exit_cells, cell_frame_t, 0) {
    LLG_PROBE_BEGIN(cell_frame_t, 0);
    F->packed[0] = F->packed[1] = (sv4_t)SV4_EMPTY;
    llg_value_scope_register(&F->packed_scope, F->packed, 2);
    sv4_replace(&F->packed[0], sv4_from_u64(7, 65, 0));
    F->native = 2;
    llg_value_scope_register_object(&F->native_scope, &F->native, drop_native);
    llg_rt_finish();
    LLG_CO_EXIT_CHECK(ch);
    LLG_PROBE_DONE();
}

static void mixed_scopes(void) {
    llg_value_scope_t packed_node;
    sv4_t packed = (sv4_t)SV4_EMPTY;
    llg_value_scope_t native_node;
    unsigned native = 1;
    llg_value_scope_t* mark = llg_value_scope_mark();
    llg_value_scope_register_object(&native_node, &native, drop_native);
    llg_value_scope_t* inner = llg_value_scope_mark();
    llg_value_scope_register(&packed_node, &packed, 1);
    llg_value_scope_t* heap = llg_value_scope_begin(1);
    sv4_t* retained = llg_value_scope_values(heap);
    sv4_replace(&packed, sv4_from_u64(42, 65, 0));
    llg_nba_after(retained, packed, 1);
    llg_value_scopes_end_since(inner);
    CHECK(packed.bits == NULL && !packed_node.active);
    CHECK(heap->references == 1 && !heap->active);
    CHECK(value_scope_index_find(retained) == heap);
    CHECK(value_scope_index_find(&packed) == NULL);
    CHECK(value_scope_index_find(&native) == &native_node);
    g.now = 1;
    commit_nbas(LLG_REGION_NBA);
    CHECK(value_scope_index_find(retained) == NULL);
    llg_value_scopes_end_since(mark);
    CHECK(!native_node.active);
}

int main(int argc, char** argv) {
    (void)argv;
    llg_rt_init();
    if (argc > 1) {
        llg_value_scope_t node;
        sv4_t value = (sv4_t)SV4_EMPTY;
        llg_value_scope_register(&node, &value, 1);
        sv4_replace(&value, sv4_zero(1, 0));
        llg_nba_after(&value, value, 1);
        llg_value_scope_end(&node);
        return 1;
    }
    drops = order = 0;
    mixed_scopes();
    CHECK(drops == 1 && order == 1);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
    llg_rt_init();
    drops = order = 0;
    llg_spawn(&repeat_desc, "frame reuse");
    llg_rt_run();
    CHECK(drops == 17);
    CHECK(all_value_scopes == NULL && value_scope_count == 0);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
    llg_rt_init();
    drops = order = 0;
    victim = llg_spawn(&repeat_desc, "frame cancel");
    llg_spawn(&killer_desc, "killer");
    llg_rt_run();
    CHECK(drops == 2 && order == 21);
    CHECK(all_value_scopes == NULL && value_scope_count == 0);
    llg_rt_cleanup();
    CHECK(value_test_live() == 0);
    llg_rt_init();
    drops = order = 0;
    llg_spawn(&exit_cells_desc, "zero-resume exit");
    llg_rt_run();
    llg_rt_cleanup();
    CHECK(drops == 1 && order == 2);
    CHECK(value_scope_count == 0 && value_test_live() == 0);
    puts("frame value cells: OK");
    return 0;
}
