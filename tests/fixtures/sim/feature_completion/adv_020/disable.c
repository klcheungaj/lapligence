/* ADV-020: cbDisable (IEEE 1800-2009 38.36.1). Companion of disable.sv. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 on_disable(p_cb_data data) {
    PLI_INT32 type = vpi_get(vpiType, data->obj);
    const char* name = type == vpiSysTaskCall
                           ? (const char*)vpi_get_str(vpiName, vpi_handle(vpiUserSystf, data->obj))
                           : (const char*)vpi_get_str(vpiFullName, data->obj);
    vpi_printf("cbDisable %s: %s %s at %u value=%s\n", (const char*)data->user_data,
               (const char*)vpi_get_str(vpiType, data->obj), name ? name : "?",
               (unsigned)data->time->low, data->value ? "set" : "NULL");
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
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("register %s: %s level=%d code=%s\n", label, handle ? "handle" : "NULL",
               (int)level, level && info.code ? (const char*)info.code : "-");
}

static vpiHandle named(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static PLI_INT32 watch(PLI_BYTE8* data) {
    (void)data;
    add("blk", named("tb.blk"));
    add("fk", named("tb.fk"));
    add("work", named("tb.work"));
    add("outer2.inner", named("tb.outer2.inner"));
    add("selfb", named("tb.selfb"));
    add("NULL", NULL);
    add("v", named("tb.v"));
    add("v[0]", named("tb.v[0]"));
    add("module", named("tb"));
    return 0;
}

static PLI_INT32 inner(PLI_BYTE8* data) {
    (void)data;
    add("$inner call", vpi_handle(vpiSysTfCall, NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$watch_disable";
    task.calltf = watch;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$inner";
    task.calltf = inner;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
