/* SIM-043 A03: stop, resume and finish driven from VPI callbacks. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static unsigned now_of(p_cb_data data) { return data->time ? data->time->low : 0u; }

static const char* error_code(void) {
    static char code[64];
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    if (!vpi_chk_error(&info)) return "none";
    snprintf(code, sizeof(code), "%s", info.code ? (const char*)info.code : "?");
    return code;
}

static void register_cb(PLI_INT32 reason, PLI_INT32 (*routine)(p_cb_data), unsigned delay,
                        const char* label) {
    s_vpi_time time;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    time.type = vpiSimTime;
    time.high = 0;
    time.low = delay;
    data.reason = reason;
    data.cb_rtn = routine;
    data.time = &time;
    data.user_data = (PLI_BYTE8*)label;
    vpi_register_cb(&data);
}

static PLI_INT32 stop_now(p_cb_data data) {
    PLI_INT32 status = vpi_control(vpiStop, 1);
    vpi_printf("[%u] %s: vpiStop -> %d\n", now_of(data), data->user_data, (int)status);
    return 0;
}

static PLI_INT32 finish_now(p_cb_data data) {
    PLI_INT32 first = vpi_control(vpiFinish, 1);
    PLI_INT32 again = vpi_control(vpiFinish, 1);
    vpi_printf("[%u] %s: vpiFinish -> %d, again -> %d\n", now_of(data), data->user_data,
               (int)first, (int)again);
    return 0;
}

static PLI_INT32 never(p_cb_data data) {
    vpi_printf("[%u] %s: unreachable\n", now_of(data), data->user_data);
    return 0;
}

static PLI_INT32 started(p_cb_data data) {
    vpi_printf("[%u] start\n", now_of(data));
    PLI_INT32 status = vpi_control(vpiReset, 0, 0, 0);
    vpi_printf("vpiReset -> %d %s\n", (int)status, error_code());
    register_cb(cbAfterDelay, stop_now, 3, "after delay 3");
    register_cb(cbReadWriteSynch, finish_now, 7, "read-write 7");
    register_cb(cbAfterDelay, never, 9, "after delay 9");
    return 0;
}

static int end_calls;

static PLI_INT32 ended(p_cb_data data) {
    ++end_calls;
    PLI_INT32 finish = vpi_control(vpiFinish, 0);
    PLI_INT32 stop = vpi_control(vpiStop, 0);
    vpi_printf("[%u] %s call %d: vpiFinish -> %d, vpiStop -> %d\n", now_of(data), data->user_data,
               end_calls, (int)finish, (int)stop);
    s_cb_data again;
    memset(&again, 0, sizeof(again));
    again.reason = cbEndOfSimulation;
    again.cb_rtn = ended;
    vpiHandle late = vpi_register_cb(&again);
    const char* code = error_code();
    vpi_printf("late cbEndOfSimulation: %s %s\n", late ? "handle" : "null", code);
    return 0;
}

static void action(PLI_INT32 reason, PLI_INT32 (*routine)(p_cb_data), const char* label) {
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = reason;
    data.cb_rtn = routine;
    data.user_data = (PLI_BYTE8*)label;
    vpi_register_cb(&data);
}

static void startup(void) {
    action(cbStartOfSimulation, started, "start");
    action(cbEndOfSimulation, ended, "end");
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
