/* Decision A17-D7 (see A17-D7_future_disabled_start.sv). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* reason_name(PLI_INT32 reason) {
    switch (reason) {
        case cbAssertionStart: return "Start";
        case cbAssertionSuccess: return "Success";
        case cbAssertionVacuousSuccess: return "VacuousSuccess";
        case cbAssertionFailure: return "Failure";
        default: return "Disabled";
    }
}

static PLI_INT32 on_attempt(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                            p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)user_data;
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("now=%u cb_time=%u %s %s attempt=%u\n", (unsigned)now.low, (unsigned)time->low,
               (const char*)vpi_get_str(vpiName, assertion), reason_name(reason),
               (unsigned)info->attemptStartTime.low);
    return 0;
}

static PLI_INT32 d7(PLI_BYTE8* data) {
    (void)data;
    const PLI_INT32 reasons[] = {cbAssertionStart, cbAssertionSuccess, cbAssertionVacuousSuccess,
                                 cbAssertionFailure, cbAssertionDisabledEvaluation};
    vpiHandle iterator = vpi_iterate(vpiAssertion, NULL), assertion;
    while ((assertion = vpi_scan(iterator)))
        for (int i = 0; i < 5; ++i)
            vpi_register_assertion_cb(assertion, reasons[i], on_attempt, NULL);
    return 0;
}

static PLI_INT32 at_end(p_cb_data data) {
    (void)data;
    vpiHandle iterator = vpi_iterate(vpiAssertion, NULL), assertion;
    while ((assertion = vpi_scan(iterator)))
        vpi_printf("%s attempts=%d success=%d\n", (const char*)vpi_get_str(vpiName, assertion),
                   (int)vpi_get(vpiAssertAttemptCovered, assertion),
                   (int)vpi_get(vpiAssertSuccessCovered, assertion));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d7";
    task.calltf = d7;
    vpi_register_systf(&task);
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbEndOfSimulation;
    cb.cb_rtn = at_end;
    vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
