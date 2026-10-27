/* Decision A17-D2 (see A17-D2_control_callbacks.sv). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle q;

static const char* reason_name(PLI_INT32 reason) {
    switch (reason) {
        case cbAssertionStart: return "Start";
        case cbAssertionSuccess: return "Success";
        case cbAssertionFailure: return "Failure";
        case cbAssertionKill: return "Kill";
        case cbAssertionDisable: return "Disable";
        case cbAssertionEnable: return "Enable";
        case cbAssertionReset: return "Reset";
        default: return "action";
    }
}

static PLI_INT32 on_event(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                          p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)assertion;
    (void)user_data;
    if (info)
        vpi_printf("%u %s attempt=%u\n", (unsigned)time->low, reason_name(reason),
                   (unsigned)info->attemptStartTime.low);
    else
        vpi_printf("%u %s info=NULL\n", (unsigned)time->low, reason_name(reason));
    return 0;
}

static void status(const char* what, PLI_INT32 value) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("  %s -> %d level=%d\n", what, (int)value, (int)level);
}

static PLI_INT32 d2(PLI_BYTE8* data) {
    (void)data;
    q = vpi_handle_by_name((PLI_BYTE8*)"tb.q", NULL);
    const PLI_INT32 reasons[] = {cbAssertionStart,   cbAssertionSuccess,
                                 cbAssertionFailure, cbAssertionKill,
                                 cbAssertionDisable, cbAssertionEnable,
                                 cbAssertionReset};
    for (int i = 0; i < 7; ++i) vpi_register_assertion_cb(q, reasons[i], on_event, NULL);
    return 0;
}

static PLI_INT32 d2_step(PLI_BYTE8* data) {
    (void)data;
    vpiHandle args = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &value);
    vpi_release_handle(args);
    s_vpi_time at = {vpiSimTime, 0, 0, 0.0};
    switch (value.value.integer) {
        case 1:
            at.low = 5;
            status("kill attempt 5", vpi_control(vpiAssertionKill, q, &at));
            at.low = 3;
            status("kill attempt 3", vpi_control(vpiAssertionKill, q, &at));
            break;
        case 2:
            status("disable", vpi_control(vpiAssertionDisable, q));
            status("disable again", vpi_control(vpiAssertionDisable, q));
            break;
        case 3:
            status("reset", vpi_control(vpiAssertionReset, q));
            break;
        default:
            at.low = 55;
            status("disable step", vpi_control(vpiAssertionDisableStep, q, &at));
            status("enable step", vpi_control(vpiAssertionEnableStep, q, &at, vpiAssertionClockSteps));
            vpi_printf("  attempts=%d success=%d killed=%d\n", (int)vpi_get(vpiAssertAttemptCovered, q),
                       (int)vpi_get(vpiAssertSuccessCovered, q), (int)vpi_get(vpiAssertKillCovered, q));
            break;
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d2";
    task.calltf = d2;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$d2_step";
    task.calltf = d2_step;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
