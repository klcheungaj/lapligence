/* H28 positive VPI plugin. The generated model supplies vpi_user.h. */
#include "vpi_user.h"
#include <string.h>

static int consume_arguments(const char* label) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    if (!call) return 0;
    int count = 0;
    vpiHandle iterator = vpi_iterate(vpiArgument, call);
    if (!iterator) return 0;
    for (;;) {
        vpiHandle argument = vpi_scan(iterator);
        if (!argument) break;
        ++count;
        if (vpi_get(vpiSize, argument) == 1) {
            /* Compile-time descriptors carry shape, not a runtime value. */
            s_vpi_value value = {0};
            value.format = vpiScalarVal;
            vpi_get_value(argument, &value);
            vpi_printf("%s-arg=%d\n", (PLI_BYTE8*)label, value.value.scalar);
        }
        vpi_release_handle(argument);
    }
    vpi_release_handle(iterator);
    return count;
}

static PLI_INT32 start_callback(struct t_cb_data* data) {
    s_vpi_time now = {0};
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("vpi-start=%u:%u\n", now.high, now.low);
    (void)data;
    return 0;
}

static PLI_INT32 end_callback(struct t_cb_data* data) {
    vpi_printf("vpi-end\n");
    (void)data;
    return 0;
}

static PLI_INT32 probe_compile(PLI_BYTE8* user_data) {
    int count = consume_arguments("compile");
    vpi_printf("compile-args=%d\n", count);
    (void)user_data;
    return 0;
}

static PLI_INT32 probe_call(PLI_BYTE8* user_data) {
    vpiHandle value = vpi_handle_by_name((PLI_BYTE8*)"tb.value", NULL);
    vpiHandle alias = vpi_handle_by_name((PLI_BYTE8*)"tb.alias_wire", NULL);
    vpiHandle child = vpi_handle_by_name((PLI_BYTE8*)"tb.u_leaf", NULL);
    vpiHandle scope = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    vpiHandle relative_child = vpi_handle_by_name((PLI_BYTE8*)"u_leaf", scope);
    vpiHandle parent = vpi_handle(vpiParent, value);
    int child_count = 0;
    vpiHandle iterator = vpi_iterate(vpiVariables, parent);
    if (iterator) {
        while (vpi_scan(iterator)) ++child_count;
        vpi_release_handle(iterator);
    }
    s_vpi_value value_data = {0};
    value_data.format = vpiScalarVal;
    vpi_get_value(value, &value_data);
    s_vpi_value alias_data = {0};
    alias_data.format = vpiScalarVal;
    vpi_get_value(alias, &alias_data);
    vpi_printf("lookup=%d/%d parent=%s vars=%d\n", value_data.value.scalar,
               alias_data.value.scalar, vpi_get_str(vpiName, parent), child_count);
    const char* parent_definition = (const char*)vpi_get_str(vpiDefName, parent);
    const char* child_definition = (const char*)vpi_get_str(vpiDefName, child);
    const char* source_file = (const char*)vpi_get_str(vpiFile, value);
    int metadata = parent_definition && child_definition && source_file &&
                   strcmp(parent_definition, "tb") == 0 &&
                   strcmp(child_definition, "leaf") == 0 &&
                   vpi_compare_objects(child, relative_child) &&
                   vpi_get(vpiLineNo, value) == 7 &&
                   vpi_get(vpiTopModule, parent) == vpiValidTrue &&
                   vpi_get(vpiTopModule, child) == vpiValidFalse;
    vpi_printf("metadata=%d\n", metadata);

    /* Unsupported properties and stale dynamic handles must report through
     * vpi_chk_error instead of fabricating a value. */
    (void)vpi_get(vpiDirection, value);
    s_vpi_error_info error = {0};
    if (vpi_chk_error(&error))
        vpi_printf("negative=%s\n", error.code);
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpi_release_handle(call);
    (void)vpi_get(vpiType, call);
    if (vpi_chk_error(&error))
        vpi_printf("stale=%s\n", error.code);
    (void)user_data;
    return 0;
}

static PLI_INT32 sized_compile(PLI_BYTE8* user_data) {
    int count = consume_arguments("sized-compile");
    vpi_printf("sized-compile-args=%d\n", count);
    (void)user_data;
    return 0;
}

static PLI_INT32 sized_size(PLI_BYTE8* user_data) {
    (void)user_data;
    return 5;
}

static PLI_INT32 sized_call(PLI_BYTE8* user_data) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    s_vpi_value value = {0};
    value.format = vpiIntVal;
    value.value.integer = 17;
    (void)vpi_put_value(call, &value, NULL, vpiNoDelay);
    vpi_release_handle(call);
    (void)user_data;
    return 0;
}

static PLI_INT32 real_compile(PLI_BYTE8* user_data) {
    int count = consume_arguments("real-compile");
    vpi_printf("real-compile-args=%d\n", count);
    (void)user_data;
    return 0;
}

static PLI_INT32 real_call(PLI_BYTE8* user_data) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    s_vpi_value value = {0};
    value.format = vpiRealVal;
    value.value.real = 2.5;
    (void)vpi_put_value(call, &value, NULL, vpiNoDelay);
    vpi_release_handle(call);
    (void)user_data;
    return 0;
}

static void register_vpi(void) {
    s_vpi_systf_data probe = {0};
    probe.type = vpiSysTask;
    probe.tfname = (PLI_BYTE8*)"$vpi_probe";
    probe.compiletf = probe_compile;
    probe.calltf = probe_call;
    (void)vpi_register_systf(&probe);

    s_vpi_systf_data sized = {0};
    sized.type = vpiSysFunc;
    sized.sysfunctype = vpiSizedSignedFunc;
    sized.tfname = (PLI_BYTE8*)"$vpi_sized";
    sized.compiletf = sized_compile;
    sized.sizetf = sized_size;
    sized.calltf = sized_call;
    (void)vpi_register_systf(&sized);

    s_vpi_systf_data real = {0};
    real.type = vpiSysFunc;
    real.sysfunctype = vpiRealFunc;
    real.tfname = (PLI_BYTE8*)"$vpi_real";
    real.compiletf = real_compile;
    real.calltf = real_call;
    (void)vpi_register_systf(&real);

    s_cb_data start = {0};
    start.reason = cbStartOfSimulation;
    start.cb_rtn = start_callback;
    (void)vpi_register_cb(&start);
    s_cb_data end = {0};
    end.reason = cbEndOfSimulation;
    end.cb_rtn = end_callback;
    (void)vpi_register_cb(&end);
}

void (*vlog_startup_routines[])(void) = {register_vpi, NULL};
