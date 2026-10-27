/* Companion of A20-D10_disable_callback.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 on_disable(p_cb_data data) {
    vpi_printf("cbDisable %s: %s at %u value=%s\n", (const char*)data->user_data,
               (const char*)vpi_get_str(vpiType, data->obj), (unsigned)data->time->low,
               data->value ? "set" : "NULL");
    return 0;
}

static void add(const char* label, vpiHandle object) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbDisable;
    cb.cb_rtn = on_disable;
    cb.obj = object;
    cb.time = &time;
    cb.user_data = (PLI_BYTE8*)label;
    vpiHandle handle = vpi_register_cb(&cb);
    vpi_printf("register %s: %s level=%d\n", label, handle ? "handle" : "NULL",
               (int)vpi_chk_error(NULL));
}

static PLI_INT32 d10(PLI_BYTE8* data) {
    (void)data;
    add("tb.outer", vpi_handle_by_name((PLI_BYTE8*)"tb.outer", NULL));
    add("tb.outer.inner", vpi_handle_by_name((PLI_BYTE8*)"tb.outer.inner", NULL));
    add("tb.selfb", vpi_handle_by_name((PLI_BYTE8*)"tb.selfb", NULL));
    add("NULL", NULL);
    add("tb.v", vpi_handle_by_name((PLI_BYTE8*)"tb.v", NULL));
    add("tb", vpi_handle_by_name((PLI_BYTE8*)"tb", NULL));
    return 0;
}

static PLI_INT32 d10_call(PLI_BYTE8* data) {
    (void)data;
    add("$d10_call", vpi_handle(vpiSysTfCall, NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d10";
    task.calltf = d10;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$d10_call";
    task.calltf = d10_call;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
