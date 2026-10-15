/* Companion of S43-D6_value_change_dispatch.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static vpiHandle b_handle;

static vpiHandle watch(PLI_INT32 (*routine)(p_cb_data), const char* label) {
    s_vpi_time t;
    s_vpi_value v;
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    t.type = vpiSimTime;
    v.format = vpiIntVal;
    cb.reason = cbValueChange;
    cb.cb_rtn = routine;
    cb.obj = vpi_handle_by_name((PLI_BYTE8*)"tb.v", NULL);
    cb.time = &t;
    cb.value = &v;
    cb.user_data = (PLI_BYTE8*)label;
    return vpi_register_cb(&cb);
}

static PLI_INT32 show(p_cb_data data) {
    vpi_printf("%s: v=%d at %u\n", data->user_data, (int)data->value->value.integer,
               (unsigned)data->time->low);
    return 0;
}

static PLI_INT32 a(p_cb_data data) {
    show(data);
    if (data->value->value.integer == 2) {
        vpi_remove_cb(b_handle);
        watch(show, "c");
    }
    return 0;
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    watch(a, "a");
    b_handle = watch(show, "b");
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
