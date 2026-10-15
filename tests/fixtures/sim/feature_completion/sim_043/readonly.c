/* SIM-043 A02: read-only phases reject writes and zero-delay scheduling;
 * read-write callbacks write the design. Companion of readonly.sv. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

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

static vpiHandle register_time(PLI_INT32 reason, PLI_INT32 (*routine)(p_cb_data), PLI_INT32 type,
                               unsigned delay, double real_delay) {
    s_vpi_time time;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    memset(&time, 0, sizeof(time));
    time.type = type;
    time.low = delay;
    time.real = real_delay;
    data.reason = reason;
    data.cb_rtn = routine;
    data.time = &time;
    data.obj = vpi_handle_by_name("tb", NULL);
    return vpi_register_cb(&data);
}

/* $peek(v): v + 100, and an attempted write of tb.x (rejected when the
 * caller is a Postponed $strobe/$monitor evaluator). */
static PLI_INT32 peek_call(PLI_BYTE8* user) {
    (void)user;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle argument = vpi_scan(arguments);
    vpi_free_object(arguments);
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(argument, &value);
    s_vpi_value poke;
    poke.format = vpiIntVal;
    poke.value.integer = 99;
    vpi_put_value(vpi_handle_by_name("tb.x", NULL), &poke, NULL, vpiNoDelay);
    vpi_printf("  $peek: write x -> %s\n", error_code());
    value.value.integer += 100;
    vpi_put_value(call, &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 later(p_cb_data data) {
    vpi_printf("[%.1f] read-write after delay: d=%d\n", data->time->real, read_int("tb.d"));
    return 0;
}

static PLI_INT32 read_write(p_cb_data data) {
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = 5;
    vpi_put_value(vpi_handle_by_name("tb.d", NULL), &value, NULL, vpiNoDelay);
    vpi_printf("[%.1f] read-write: put d=5 (%s)\n", data->time->real, error_code());
    return 0;
}

static PLI_INT32 read_only(p_cb_data data) {
    vpiHandle d = vpi_handle_by_name("tb.d", NULL);
    s_vpi_value value;
    s_vpi_time delay;
    memset(&delay, 0, sizeof(delay));
    delay.type = vpiSimTime;
    delay.low = 10;
    value.format = vpiIntVal;
    value.value.integer = 40;
    vpi_printf("[%.1f] read-only:\n", data->time->real);
    vpi_put_value(d, &value, NULL, vpiNoDelay);
    vpi_printf("  no-delay put -> %s\n", error_code());
    vpi_put_value(d, &value, &delay, vpiInertialDelay);
    vpi_printf("  delayed put -> %s\n", error_code());
    vpi_put_value(d, &value, NULL, vpiForceFlag);
    vpi_printf("  force -> %s\n", error_code());
    vpiHandle rw = register_time(cbReadWriteSynch, later, vpiSimTime, 0, 0.0);
    const char* rw_code = error_code();
    vpi_printf("  zero-delay read-write -> %s %s\n", rw ? "handle" : "null", rw_code);
    vpiHandle ro = register_time(cbReadOnlySynch, later, vpiSimTime, 0, 0.0);
    const char* ro_code = error_code();
    vpi_printf("  zero-delay read-only -> %s %s\n", ro ? "handle" : "null", ro_code);
    vpiHandle next = register_time(cbReadWriteSynch, later, vpiScaledRealTime, 0, 1.0);
    const char* next_code = error_code();
    vpi_printf("  read-write after 1.0 -> %s %s\n", next ? "handle" : "null", next_code);
    vpi_printf("  d=%d\n", read_int("tb.d"));
    return 0;
}

static PLI_INT32 real_change(p_cb_data data) {
    vpi_printf("[%.1f] value: r=%.2f\n", data->time->real, data->value->value.real);
    return 0;
}

static PLI_INT32 started(p_cb_data data) {
    (void)data;
    s_vpi_time time;
    s_vpi_value value;
    s_cb_data change;
    memset(&change, 0, sizeof(change));
    time.type = vpiScaledRealTime;
    value.format = vpiRealVal;
    change.reason = cbValueChange;
    change.cb_rtn = real_change;
    change.obj = vpi_handle_by_name("tb.r", NULL);
    change.time = &time;
    change.value = &value;
    vpi_register_cb(&change);
    register_time(cbReadWriteSynch, read_write, vpiScaledRealTime, 0, 2.5);
    register_time(cbReadOnlySynch, read_only, vpiScaledRealTime, 0, 2.5);
    return 0;
}

static void startup(void) {
    s_vpi_systf_data peek = {vpiSysFunc, vpiIntFunc, "$peek", peek_call, NULL, NULL, NULL};
    vpi_register_systf(&peek);
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = cbStartOfSimulation;
    data.cb_rtn = started;
    vpi_register_cb(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
