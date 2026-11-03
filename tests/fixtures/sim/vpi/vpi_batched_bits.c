#include "vpi_user.h"
#include <string.h>

static int formal_changes;

static PLI_INT32 vector_changed(p_cb_data data) {
    s_vpi_time now = {0};
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    if (now.low == 0 && now.high == 0) return 0;
    vpi_printf("%u q=%s\n", (unsigned)now.low, data->value->value.str);
    return 0;
}

static PLI_INT32 formal_changed(p_cb_data data) {
    (void)data;
    s_vpi_time now = {0};
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    if (now.low != 0 || now.high != 0) ++formal_changes;
    return 0;
}

static void watch(const char* name, PLI_INT32 (*routine)(p_cb_data), PLI_INT32 format) {
    static s_vpi_time time = {vpiSuppressTime, 0, 0, 0.0};
    static s_vpi_value values[2];
    s_vpi_value* value = &values[routine == vector_changed];
    value->format = format;
    s_cb_data data = {0};
    data.reason = cbValueChange;
    data.cb_rtn = routine;
    data.obj = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    data.time = &time;
    data.value = value;
    (void)vpi_register_cb(&data);
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    watch("tb.q", vector_changed, vpiBinStrVal);
    watch("tb.g[3].s.q", formal_changed, vpiSuppressVal);
    return 0;
}

static PLI_INT32 end(p_cb_data data) {
    (void)data;
    vpi_printf("changes of tb.g[3].s.q: %d\n", formal_changes);
    return 0;
}

static void register_callbacks(void) {
    s_cb_data data = {0};
    data.reason = cbStartOfSimulation;
    data.cb_rtn = start;
    (void)vpi_register_cb(&data);
    data.reason = cbEndOfSimulation;
    data.cb_rtn = end;
    (void)vpi_register_cb(&data);
}

void (*vlog_startup_routines[])(void) = {register_callbacks, NULL};
