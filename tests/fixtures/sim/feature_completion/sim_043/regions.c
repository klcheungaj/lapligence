/* SIM-043 A01: callback trace of regions.sv. Every callback prints its
 * reason, the time it received and, for value changes, the value. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle a_first;
static int next_count;

static unsigned now_of(p_cb_data data) { return data->time ? data->time->low : 0u; }

static const char* error_code(void) {
    static char code[64];
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    if (!vpi_chk_error(&info)) return "none";
    snprintf(code, sizeof(code), "%s", info.code ? (const char*)info.code : "?");
    return code;
}

static int read_int(const char* name) {
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)name, NULL), &value);
    return (int)value.value.integer;
}

static vpiHandle on_time(PLI_INT32 reason, PLI_INT32 (*routine)(p_cb_data), unsigned delay,
                         const char* label) {
    static s_vpi_time time;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    time.type = vpiSimTime;
    time.high = 0;
    time.low = delay;
    data.reason = reason;
    data.cb_rtn = routine;
    data.time = &time;
    data.user_data = (PLI_BYTE8*)label;
    return vpi_register_cb(&data);
}

static vpiHandle on_change(vpiHandle object, PLI_INT32 format, PLI_INT32 (*routine)(p_cb_data),
                           const char* label) {
    s_vpi_time time;
    s_vpi_value value;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    time.type = vpiSimTime;
    value.format = format;
    data.reason = cbValueChange;
    data.cb_rtn = routine;
    data.obj = object;
    data.time = &time;
    data.value = &value;
    data.user_data = (PLI_BYTE8*)label;
    return vpi_register_cb(&data);
}

static PLI_INT32 time_cb(p_cb_data data) {
    vpi_printf("[%u] %s\n", now_of(data), data->user_data);
    return 0;
}

static PLI_INT32 end_of_time(p_cb_data data) {
    vpi_printf("[%u] %s a=%d b=%d c=%d\n", now_of(data), data->user_data, read_int("tb.a"),
               read_int("tb.b"), read_int("tb.c"));
    return 0;
}

static PLI_INT32 read_write(p_cb_data data) {
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = 7;
    vpi_put_value(vpi_handle_by_name("tb.c", NULL), &value, NULL, vpiNoDelay);
    vpi_printf("[%u] %s: put c=7 (%s)\n", now_of(data), data->user_data, error_code());
    return 0;
}

static PLI_INT32 read_only(p_cb_data data) {
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = 9;
    vpi_put_value(vpi_handle_by_name("tb.c", NULL), &value, NULL, vpiNoDelay);
    const char* put = error_code();
    vpi_printf("[%u] %s: put c=9 -> %s, c=%d\n", now_of(data), data->user_data, put,
               read_int("tb.c"));
    return 0;
}

static PLI_INT32 next_time(p_cb_data data) {
    vpi_printf("[%u] %s #%d\n", now_of(data), data->user_data, ++next_count);
    /* Recurring use re-registers from the callback (one-shot reason). */
    if (next_count < 2) on_time(cbNextSimTime, next_time, 0, "pre-active: cbNextSimTime");
    return 0;
}

static PLI_INT32 change(p_cb_data data) {
    if (data->value->format == vpiIntVal)
        vpi_printf("[%u] %s=%d\n", now_of(data), data->user_data, (int)data->value->value.integer);
    else
        vpi_printf("[%u] %s=%s index=%d\n", now_of(data), data->user_data, data->value->value.str,
                   (int)data->index);
    return 0;
}

/* The first change of a: register a replacement watch (it does not see this
 * change) and remove this callback while it runs. */
static PLI_INT32 a_once(p_cb_data data) {
    vpi_printf("[%u] %s=%s\n", now_of(data), data->user_data, data->value->value.str);
    on_change(data->obj, vpiIntVal, change, "value: a (second watch)");
    vpi_printf("[%u] remove first watch: %d\n", now_of(data), (int)vpi_remove_cb(a_first));
    return 0;
}

static PLI_INT32 compiled(p_cb_data data) {
    vpi_printf("[%u] cbEndOfCompile\n", now_of(data));
    s_cb_data again;
    memset(&again, 0, sizeof(again));
    again.reason = cbEndOfCompile;
    again.cb_rtn = compiled;
    vpiHandle late = vpi_register_cb(&again);
    vpi_printf("late cbEndOfCompile: %s %s\n", late ? "handle" : "null", error_code());
    return 0;
}

static PLI_INT32 started(p_cb_data data) {
    vpi_printf("[%u] cbStartOfSimulation\n", now_of(data));
    a_first = on_change(vpi_handle_by_name("tb.a", NULL), vpiBinStrVal, a_once, "value: a");
    on_change(vpi_handle_by_name("tb.b", NULL), vpiIntVal, change, "value: b");
    on_change(vpi_handle_by_name("tb.c[0]", NULL), vpiBinStrVal, change, "value: c[0]");
    on_change(vpi_handle_by_name("tb.mem[2]", NULL), vpiHexStrVal, change, "value: mem[2]");
    on_time(cbAtStartOfSimTime, time_cb, 5, "pre-active: cbAtStartOfSimTime");
    on_time(cbNBASynch, time_cb, 5, "pre-nba: cbNBASynch");
    on_time(cbReadWriteSynch, read_write, 5, "post-nba: cbReadWriteSynch");
    on_time(cbAtEndOfSimTime, end_of_time, 5, "pre-postponed: cbAtEndOfSimTime");
    on_time(cbReadOnlySynch, read_only, 5, "postponed: cbReadOnlySynch");
    on_time(cbNextSimTime, next_time, 0, "pre-active: cbNextSimTime");
    on_time(cbAfterDelay, time_cb, 10, "pre-active: cbAfterDelay");
    return 0;
}

static PLI_INT32 ended(p_cb_data data) {
    vpi_printf("[%u] cbEndOfSimulation a=%d\n", now_of(data), read_int("tb.a"));
    return 0;
}

static void action(PLI_INT32 reason, PLI_INT32 (*routine)(p_cb_data)) {
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = reason;
    data.cb_rtn = routine;
    vpi_register_cb(&data);
}

static void startup(void) {
    action(cbEndOfCompile, compiled);
    action(cbStartOfSimulation, started);
    action(cbEndOfSimulation, ended);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
