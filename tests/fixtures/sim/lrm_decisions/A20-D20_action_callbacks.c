/* Companion of A20-D20_action_callbacks.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <signal.h>
#include <string.h>

static PLI_INT32 on_action(p_cb_data data) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("  %s at %d: level=%d state=%d index=%s\n", (const char*)data->user_data,
               (int)now.low, (int)level, level ? (int)info.state : 0,
               data->index == SIGTERM ? "SIGTERM" : data->index ? "other" : "0");
    if (data->reason == cbPLIError) (void)vpi_get(vpiSize, NULL);
    return 0;
}

static void add(const char* label, PLI_INT32 reason, PLI_INT32 index) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = reason;
    cb.cb_rtn = on_action;
    cb.time = &time;
    cb.index = index;
    cb.user_data = (PLI_BYTE8*)label;
    vpiHandle handle = vpi_register_cb(&cb);
    vpi_printf("register %s: %s level=%d\n", label, handle ? "handle" : "NULL",
               (int)vpi_chk_error(NULL));
}

static PLI_INT32 d20(PLI_BYTE8* data) {
    (void)data;
    add("cbError", cbError, 0);
    add("cbSignal", cbSignal, SIGTERM);
    add("cbPLIError", cbPLIError, 0);
    return 0;
}

static PLI_INT32 d20_fail(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("failing call\n");
    PLI_INT32 size = vpi_get(vpiSize, NULL);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("vpi_get(vpiSize, NULL) = %d level=%d\n", (int)size, (int)level);
    raise(SIGTERM);
    vpi_printf("after raise\n");
    return 0;
}

static void registration(void) {
    static const char* names[] = {"$d20", "$d20_fail"};
    PLI_INT32 (*routines[])(PLI_BYTE8*) = {d20, d20_fail};
    for (int i = 0; i < 2; ++i) {
        s_vpi_systf_data task;
        memset(&task, 0, sizeof(task));
        task.type = vpiSysTask;
        task.tfname = (PLI_BYTE8*)names[i];
        task.calltf = routines[i];
        vpi_register_systf(&task);
    }
}

void (*vlog_startup_routines[])(void) = {registration, 0};
