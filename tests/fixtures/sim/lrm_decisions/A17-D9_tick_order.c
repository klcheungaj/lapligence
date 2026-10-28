/* Decision A17-D9 (see A17-D9_tick_order.sv). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* reason_name(PLI_INT32 reason) {
    switch (reason) {
        case cbAssertionStart: return "Start";
        case cbAssertionSuccess: return "Success";
        case cbAssertionVacuousSuccess: return "VacuousSuccess";
        case cbAssertionFailure: return "Failure";
        default: return "Kill";
    }
}

static PLI_INT32 on_attempt(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                            p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)user_data;
    vpi_printf("%u %s %s attempt=%u\n", (unsigned)time->low,
               (const char*)vpi_get_str(vpiFullName, assertion), reason_name(reason),
               (unsigned)info->attemptStartTime.low);
    return 0;
}

static PLI_INT32 d9(PLI_BYTE8* data) {
    (void)data;
    const PLI_INT32 reasons[] = {cbAssertionStart, cbAssertionSuccess, cbAssertionVacuousSuccess,
                                 cbAssertionFailure};
    vpiHandle iterator = vpi_iterate(vpiAssertion, NULL), assertion;
    while ((assertion = vpi_scan(iterator)))
        for (int i = 0; i < 4; ++i)
            vpi_register_assertion_cb(assertion, reasons[i], on_attempt, NULL);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d9";
    task.calltf = d9;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
