// llg-generated C11 model for design `tb`
#define LLG_MODEL_VALUE_ABI 4
#define LLG_MODEL_VALUE_BACKEND 0
#define LLG_MODEL_COMPACT_KERNELS 0
#define LLG_MODEL_PROCESS_ABI 3
#include "llg_rt.h"
#if LLG_MODEL_PROCESS_ABI != LLG_PROCESS_ABI_VERSION
#error "generated model process ABI does not match llg_rt.h"
#endif
_Static_assert(LLG_MODEL_VALUE_BACKEND == LLG_SV4_USE_GMP, "regenerate model: incompatible value backend");
_Static_assert(LLG_MODEL_COMPACT_KERNELS == LLG_SV4_GMP_KERNELS, "regenerate model: incompatible compact kernels");
#include "llg_random.h"
#include "llg_vpi.h"
_Static_assert(LLG_MODEL_VALUE_ABI == LLG_VALUE_ABI_VERSION, "regenerate model: incompatible value ownership ABI");
#if UINTPTR_MAX == UINT64_MAX
_Static_assert(sizeof(sv4_t) == 32 && _Alignof(sv4_t) == 8, "selected packed descriptor layout mismatch");
#endif
#include "llg_container.h"
#include "llg_string.h"

#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>

static void llg_model_constants_init(void);
static void llg_model_constants_destroy(void);
static llg_co_status_t p_tb_proc_0(llg_co_frame_t* co, llg_co_chain_t* ch);
static const llg_co_desc_t p_tb_proc_0_desc;
typedef struct {
    llg_co_frame_t co;
    llg_value_scope_t* _llg_frame_base;
    llg_value_scope_t* _llg_temp_scope;
    sv4_t* _llg_t;
    llg_value_scope_t* _llg_mark_0;
    llg_value_scope_t* _llg_mark_1;
    llg_value_scope_t* _llg_mark_2;
} p_tb_proc_0_frame_t;
LLG_CO_ROOT_FRAME_OK(p_tb_proc_0_frame_t);
/* signals start all-X; driven by processes and link processes */
static void llg_owned_string_drop(void* p) { llg_string_destroy((llg_string_t*)p); }
static void llg_owned_process_drop(void* p) { llg_process_release(*(llg_process_handle_t**)p); }


/* owned hierarchy catalog for the bounded VPI bridge */
static llg_vpi_model_object_t llg_vpi_objects[] = {
    { vpiModule, "tb", "tb", "tb", "/workspaces/agent_workspace/lapligence-wt/sim-002/tests/fixtures/sim/feature_completion/sim_002/timeformat_reset_defaults.v", 4, 0, 0, 0, 0, NULL, NULL, NULL, 1000000ULL },
};
static const size_t llg_vpi_object_count = sizeof(llg_vpi_objects) / sizeof(llg_vpi_objects[0]);


#if UINTPTR_MAX == UINT64_MAX
_Static_assert(sizeof(p_tb_proc_0_frame_t) <= 56, "coroutine frame exceeds selected layout estimate");
#endif
static const llg_co_site_t p_tb_proc_0_desc_sites[2] = {
    {0},
    { NULL, 0, 0, "/workspaces/agent_workspace/lapligence-wt/sim-002/tests/fixtures/sim/feature_completion/sim_002/timeformat_reset_defaults.v:4:66" },
};
static const llg_co_desc_t p_tb_proc_0_desc = { p_tb_proc_0, "tb.initial", sizeof(p_tb_proc_0_frame_t), p_tb_proc_0_desc_sites, 2, 0 };
static llg_co_status_t p_tb_proc_0(llg_co_frame_t* co, llg_co_chain_t* ch) {
    p_tb_proc_0_frame_t* F = (p_tb_proc_0_frame_t*)co;
    sv4_t* _llg_t;
    LLG_CO_DISPATCH_BEGIN(co)
    LLG_CO_RESUME_CASE(1)
    LLG_CO_DISPATCH_END(co)
    F->_llg_frame_base = llg_value_scope_mark();
    F->_llg_temp_scope = llg_value_scope_begin(3);
    F->_llg_t = llg_value_scope_values(F->_llg_temp_scope);
    (void)F->_llg_t;
    _llg_t = F->_llg_t;
    goto _llg_exec_0_b0;
    _llg_exec_0_b0: ;
    {
    F->_llg_mark_0 = llg_value_scope_mark(); 
    {
    F->_llg_mark_1 = llg_value_scope_mark(); 
    {
    F->_llg_mark_2 = llg_value_scope_mark(); 
    {
    llg_value_scope_t* _llg_mark_3 = llg_value_scope_mark(); 
    sv4_from_masks_to(&_llg_t[0], 4294967287ULL, 0ULL, 0ULL, 32, 1);
    sv4_from_masks_to(&_llg_t[1], 2ULL, 0ULL, 0ULL, 32, 1);
    llg_value_scope_t* _llg_native_scope_4 = llg_value_scope_begin_object(sizeof(llg_string_t), llg_owned_string_drop); 
    llg_string_t* _llg_native_5 = (llg_string_t*)llg_value_scope_object(_llg_native_scope_4); 
    llg_string_bytes_to(_llg_native_5, "\040\156\163", 3);
    sv4_from_masks_to(&_llg_t[2], 0ULL, 0ULL, 0ULL, 32, 1);
    llg_timeformat(_llg_t[0], _llg_t[1], llg_string_take(_llg_native_5), _llg_t[2]);
    llg_value_scope_end(_llg_native_scope_4);
    sv4_destroy(&_llg_t[0]);
    sv4_destroy(&_llg_t[1]);
    sv4_destroy(&_llg_t[2]);
    llg_value_scopes_end_since(_llg_mark_3);
    }
    {
    llg_value_scope_t* _llg_mark_6 = llg_value_scope_mark(); 
    sv4_from_masks_to(&_llg_t[0], 4294967284ULL, 0ULL, 0ULL, 32, 1);
    sv4_from_masks_to(&_llg_t[1], 0ULL, 0ULL, 0ULL, 32, 0);
    llg_value_scope_t* _llg_native_scope_7 = llg_value_scope_begin_object(sizeof(llg_string_t), llg_owned_string_drop); 
    llg_string_t* _llg_native_8 = (llg_string_t*)llg_value_scope_object(_llg_native_scope_7); 
    llg_string_bytes_to(_llg_native_8, "", 0);
    sv4_from_masks_to(&_llg_t[2], 20ULL, 0ULL, 0ULL, 32, 0);
    llg_timeformat(_llg_t[0], _llg_t[1], llg_string_take(_llg_native_8), _llg_t[2]);
    llg_value_scope_end(_llg_native_scope_7);
    sv4_destroy(&_llg_t[0]);
    sv4_destroy(&_llg_t[1]);
    sv4_destroy(&_llg_t[2]);
    llg_value_scopes_end_since(_llg_mark_6);
    }
    LLG_CO_AWAIT(co, ch, 1, llg_arm_time(LLG_CO_OWNER(ch, llg_proc_t), 1000ULL));
    _llg_t = F->_llg_t;
    {
    llg_value_scope_t* _llg_mark_9 = llg_value_scope_mark(); 
    double _llg_scalar_10 = ((double)llg_time() * 1000.0 / 1000000.0); 
    llg_fmt_arg_t _llg_format_args_11[1];
    memset(_llg_format_args_11, 0, sizeof(_llg_format_args_11));
    _llg_format_args_11[0].time_unit_fs = 1000000ULL;
    _llg_format_args_11[0].kind = LLG_FMT_REAL;
    _llg_format_args_11[0].value.real = _llg_scalar_10;
    (void)_llg_scalar_10;
    llg_display_typed("%0t", _llg_format_args_11, 1, "tb");
    llg_value_scopes_end_since(_llg_mark_9);
    }
    {
    llg_value_scope_t* _llg_mark_12 = llg_value_scope_mark(); 
    llg_rt_finish_with_level(1, "tb:4:97");
    LLG_CO_EXIT_CHECK(ch);
    llg_value_scopes_end_since(_llg_mark_12);
    }
    LLG_CO_EXIT_CHECK(ch);
    llg_value_scopes_end_since(F->_llg_mark_2);
    }
    LLG_CO_EXIT_CHECK(ch);
    llg_value_scopes_end_since(F->_llg_mark_1);
    }
    LLG_CO_EXIT_CHECK(ch);
    llg_value_scopes_end_since(F->_llg_mark_0);
    }
    goto _llg_return;
    goto _llg_return;
    _llg_return: ;
    llg_value_scopes_end_since(F->_llg_frame_base);
    #ifdef LLG_CO_DEBUG
    LLG_CO_DEBUG_POISON_FRAME(F, sizeof(*F));
    #endif
    return LLG_CO_DONE;

}
static int llg_model_assertions_init(void) {
    return !llg_rt_failed();
}

static void llg_model_storage_defaults(void) {
    llg_model_constants_init();
}

static void llg_model_storage_destroy(void) {
    llg_model_constants_destroy();
}

static void llg_model_initializers(void) {
    llg_value_scope_t* _llg_frame_base = llg_value_scope_mark();
    llg_value_scope_t* _llg_temp_scope = llg_value_scope_begin(0);
    sv4_t* _llg_t = llg_value_scope_values(_llg_temp_scope);
    (void)_llg_t;
    llg_value_scopes_end_since(_llg_frame_base);

}

static void llg_model_constants_init(void) {
}
static void llg_model_constants_destroy(void) {
}
/* start: 0=ready, 1=error; advance: 0=complete, 1=error, 2=suspended.
* A suspended model retains all owners until another advance or close.
* Define LLG_MODEL_NO_MAIN to drive these entry points from a host. */
static int llg_model_live, llg_model_done, llg_model_status;
int llg_model_close(void);
int llg_model_start(int argc, char** argv) {
    llg_value_require_abi();
    if (llg_model_live) return 1;
    llg_model_live = 1;
    llg_model_done = llg_model_status = 0;
    llg_rt_init_with_args_and_precision(argc, argv, 1000ULL);
    if (llg_rt_failed()) goto start_failed;
    llg_model_storage_defaults();
    llg_model_initializers();
    if (llg_rt_failed()) goto start_failed;
    (void)llg_owned_string_drop; (void)llg_owned_process_drop;
    if (!llg_model_assertions_init()) goto start_failed;
    if (!llg_vpi_model_init("tb", llg_vpi_objects, llg_vpi_object_count) || !llg_vpi_startup()) goto start_failed;
    llg_vpi_start_simulation();
    if (llg_vpi_failed()) goto start_failed;
    llg_spawn_in_region(&p_tb_proc_0_desc, "tb.initial", LLG_REGION_ACTIVE);
    return 0;
start_failed:
    (void)llg_model_close();
    return 1;
}

int llg_model_advance(void) {
    if (!llg_model_live) return 1;
    if (llg_model_done) return llg_model_status;
    if (llg_rt_is_suspended() && !llg_rt_resume()) return 1;
    llg_rt_run();
    if (llg_rt_is_suspended()) return 2;
    llg_vpi_end_simulation();
    llg_model_done = 1;
    llg_model_status = (llg_rt_failed() || llg_vpi_failed()) ? 1 : 0;
    return llg_model_status;
}

int llg_model_close(void) {
    int status = 0;
    if (!llg_model_live) return 0;
    llg_vpi_shutdown();
    llg_rt_cleanup();
    llg_model_storage_destroy();
    llg_model_live = llg_model_done = llg_model_status = 0;
    return status;
}

#ifndef LLG_MODEL_NO_MAIN
int main(int argc, char** argv) {
    int status = llg_model_start(argc, argv);
    if (status == 0) {
        status = llg_model_advance();
        if (status == 2) status = 0; /* CLI exit-policy stop, not a model error. */
    }
    if (llg_model_close() != 0) status = 1;
    return status;
}
#endif
