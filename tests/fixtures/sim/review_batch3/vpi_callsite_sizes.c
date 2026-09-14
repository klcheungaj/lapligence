/* R23: two call sites of one registration must retain separate return shapes.
 * Direct bridge input deliberately avoids frontend argument-width coercions.
 * This source was not compiled or executed during patch preparation. */
#include "llg_vpi.h"
#include <stdio.h>
#include <string.h>

static int compile_count, size_count, run_count;
static PLI_INT32 check_compile(PLI_BYTE8* unused) {
    (void)unused;
    ++compile_count;
    return 0;
}
static PLI_INT32 size_call(PLI_BYTE8* unused) {
    (void)unused;
    ++size_count;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle iterator = vpi_iterate(vpiArgument, call);
    vpiHandle arg = vpi_scan(iterator);
    return arg ? vpi_get(vpiSize, arg) : 0;
}
static PLI_INT32 run_call(PLI_BYTE8* unused) {
    (void)unused;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle iterator = vpi_iterate(vpiArgument, call);
    vpiHandle arg = vpi_scan(iterator);
    if (!arg || vpi_get(vpiSize, call) != vpi_get(vpiSize, arg)) return 1;
    s_vpi_value value = {0};
    value.format = vpiIntVal;
    value.value.integer = 0x1234;
    (void)vpi_put_value(call, &value, NULL, vpiNoDelay);
    ++run_count;
    return vpi_chk_error(NULL) ? 1 : 0;
}
static int fail(const char* text) {
    fprintf(stderr, "VPI callsite probe: %s\n", text);
    return 1;
}
int main(void) {
    llg_rt_init();
    llg_vpi_model_object_t objects[] = {
        {.type = vpiModule, .name = "tb", .full_name = "tb",
         .definition_name = "tb", .file = "probe.sv", .line = 1,
         .time_unit_fs = 1000000u}
    };
    if (!llg_vpi_model_init("tb", objects, 1)) return fail("model init");
    s_vpi_systf_data registration = {0};
    registration.type = vpiSysFunc;
    registration.sysfunctype = vpiSizedFunc;
    registration.tfname = (PLI_BYTE8*)"$varying_size";
    registration.compiletf = check_compile;
    registration.sizetf = size_call;
    registration.calltf = run_call;
    if (!vpi_register_systf(&registration)) return fail("registration");
    const llg_vpi_compile_arg_t shape8[] = {{8, 0, 0}};
    const llg_vpi_compile_arg_t shape16[] = {{16, 0, 0}};
    if (!llg_vpi_compile_call_site(4, "$varying_size", shape8, 1, 1000000u) ||
        !llg_vpi_compile_call_site(9, "$varying_size", shape16, 1, 1000u))
        return fail("compilation");
    llg_vpi_arg_t arg8 = {0}, arg16 = {0};
    arg8.kind = arg16.kind = LLG_FMT_PACKED;
    arg8.width = 8; arg16.width = 16;
    arg8.packed = sv4_from_u64(1, 8, 0);
    arg16.packed = sv4_from_u64(1, 16, 0);
    sv4_t a = llg_vpi_call_function_site(4, "$varying_size", &arg8, 1, 32, 0);
    sv4_t b = llg_vpi_call_function_site(9, "$varying_size", &arg16, 1, 32, 0);
    sv4_t c = llg_vpi_call_function_site(4, "$varying_size", &arg8, 1, 32, 0);
    if (a.width != 8 || b.width != 16 || c.width != 8 ||
        sv4_to_u64(a) != 0x34 || sv4_to_u64(b) != 0x1234 || sv4_to_u64(c) != 0x34)
        return fail("callsite result widths/values");
    if (compile_count != 2 || size_count != 2 || run_count != 3 || llg_vpi_failed())
        return fail("callback count/state");
    llg_vpi_shutdown();
    llg_rt_cleanup();
    puts("vpi callsite sizes ok");
    return 0;
}
