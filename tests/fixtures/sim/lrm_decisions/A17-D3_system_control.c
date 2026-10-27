/* Decision A17-D3 (see A17-D3_system_control.sv). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle t1, s1, u, t1_start;

static PLI_INT32 on_attempt(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                            p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)user_data;
    const char* name = reason == cbAssertionStart     ? "Start"
                       : reason == cbAssertionSuccess ? "Success"
                                                      : "Kill";
    if (info)
        vpi_printf("%u %s %s attempt=%u\n", (unsigned)time->low,
                   (const char*)vpi_get_str(vpiName, assertion), name,
                   (unsigned)info->attemptStartTime.low);
    else
        vpi_printf("%u %s %s info=NULL\n", (unsigned)time->low,
                   (const char*)vpi_get_str(vpiName, assertion), name);
    return 0;
}

static PLI_INT32 on_system(p_cb_data data) {
    const char* name = "?";
    switch (data->reason) {
        case cbStartOfSimulation: name = "cbStartOfSimulation"; break;
        case cbEndOfSimulation: name = "cbEndOfSimulation"; break;
        case cbAssertionSysInitialized: name = "SysInitialized"; break;
        case cbAssertionSysOn: name = "SysOn"; break;
        case cbAssertionSysOff: name = "SysOff"; break;
        case cbAssertionSysKill: name = "SysKill"; break;
        case cbAssertionSysReset: name = "SysReset"; break;
        case cbAssertionSysEnd: name = "SysEnd"; break;
    }
    vpi_printf("%u %s\n", (unsigned)data->time->low, name);
    if (data->reason == cbEndOfSimulation)
        vpi_printf("t1 attempts=%d success=%d killed=%d; s1 attempts=%d success=%d\n",
                   (int)vpi_get(vpiAssertAttemptCovered, t1), (int)vpi_get(vpiAssertSuccessCovered, t1),
                   (int)vpi_get(vpiAssertKillCovered, t1), (int)vpi_get(vpiAssertAttemptCovered, s1),
                   (int)vpi_get(vpiAssertSuccessCovered, s1));
    return 0;
}

static void status(const char* what, PLI_INT32 value) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("  %s -> %d level=%d\n", what, (int)value, (int)level);
}

static PLI_INT32 d3(PLI_BYTE8* data) {
    (void)data;
    t1 = vpi_handle_by_name((PLI_BYTE8*)"tb.t1", NULL);
    s1 = vpi_handle_by_name((PLI_BYTE8*)"tb.u.s1", NULL);
    u = vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL);
    t1_start = vpi_register_assertion_cb(t1, cbAssertionStart, on_attempt, NULL);
    vpi_register_assertion_cb(t1, cbAssertionSuccess, on_attempt, NULL);
    vpi_register_assertion_cb(t1, cbAssertionKill, on_attempt, NULL);
    vpi_register_assertion_cb(s1, cbAssertionStart, on_attempt, NULL);
    return 0;
}

static PLI_INT32 d3_step(PLI_BYTE8* data) {
    (void)data;
    vpiHandle args = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &value);
    vpi_release_handle(args);
    switch (value.value.integer) {
        case 1: status("system off tb.u", vpi_control(vpiAssertionSysOff, u)); break;
        case 2:
            status("system on", vpi_control(vpiAssertionSysOn, NULL));
            status("system kill tb.t1", vpi_control(vpiAssertionSysKill, t1));
            break;
        case 3:
            status("system reset", vpi_control(vpiAssertionSysReset, NULL));
            status("system end tb.u", vpi_control(vpiAssertionSysEnd, u));
            break;
        default:
            status("system end", vpi_control(vpiAssertionSysEnd, NULL));
            status("remove a removed callback", vpi_remove_cb(t1_start));
            status("system on after the end", vpi_control(vpiAssertionSysOn, NULL));
            status("place a callback after the end",
                   vpi_register_assertion_cb(t1, cbAssertionStart, on_attempt, NULL) != NULL);
            break;
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d3";
    task.calltf = d3;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$d3_step";
    task.calltf = d3_step;
    vpi_register_systf(&task);
    const PLI_INT32 reasons[] = {cbStartOfSimulation, cbAssertionSysInitialized, cbAssertionSysOn,
                                 cbAssertionSysOff,   cbAssertionSysKill,        cbAssertionSysReset,
                                 cbAssertionSysEnd,   cbEndOfSimulation};
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    for (int i = 0; i < 8; ++i) {
        s_cb_data cb;
        memset(&cb, 0, sizeof(cb));
        cb.reason = reasons[i];
        cb.cb_rtn = on_system;
        cb.time = &time;
        vpi_register_cb(&cb);
    }
}

void (*vlog_startup_routines[])(void) = {registration, 0};
