/* Companion of S43-D7_select_and_element_watch.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static vpiHandle watch(const char* name, PLI_INT32 (*routine)(p_cb_data)) {
    s_vpi_time t;
    s_vpi_value v;
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    t.type = vpiSimTime;
    v.format = vpiBinStrVal;
    cb.reason = cbValueChange;
    cb.cb_rtn = routine;
    cb.obj = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    cb.time = &t;
    cb.value = &v;
    cb.user_data = (PLI_BYTE8*)name;
    return vpi_register_cb(&cb);
}

static PLI_INT32 show(p_cb_data data) {
    vpi_printf("%s=%s index=%d at %u\n", data->user_data, data->value->value.str,
               (int)data->index, (unsigned)data->time->low);
    return 0;
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    watch("tb.v[7:4]", show);
    watch("tb.m[4]", show);
    vpiHandle whole = watch("tb.m", show);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("whole array: %s, error %d\n", whole ? "handle" : "NULL", (int)level);
    return 0;
}

static void startup(void) {
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbStartOfSimulation;
    cb.cb_rtn = start;
    vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
