/* ADV-017 A01/A02: assertion callbacks, control and counters (IEEE
 * 1800-2009 39.4.2, 39.5.2, 40.5.3). Companion of lifecycle.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

#define MAX_ASSERTIONS 8

enum { R_START, R_SUCCESS, R_VACUOUS, R_FAILURE, R_DISABLED, R_KILL, R_COUNT };

static const PLI_INT32 reasons[R_COUNT] = {
    cbAssertionStart,   cbAssertionSuccess,           cbAssertionVacuousSuccess,
    cbAssertionFailure, cbAssertionDisabledEvaluation, cbAssertionKill};
static const char* const reason_names[R_COUNT] = {"Start",   "Success",  "VacuousSuccess",
                                                  "Failure", "Disabled", "Kill"};

typedef struct {
    vpiHandle handle;
    char name[64];
    int tally[R_COUNT];
    int pass_on, fail_on;   /* the HDL action of this outcome runs */
    int pass_expected, fail_expected;
    int trace;
} assertion_t;

static assertion_t assertions[MAX_ASSERTIONS];
static int assertion_count;
static assertion_t* a_req;
static vpiHandle second_start;
static int second_start_calls;

static void check(const char* what) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    if (level) vpi_printf("%s: error level=%d code=%s\n", what, (int)level, (const char*)info.code);
}

static PLI_INT32 on_attempt(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                            p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    assertion_t* a = (assertion_t*)user_data;
    int r = 0;
    while (r < R_COUNT && reasons[r] != reason) r++;
    if (r == R_COUNT || !vpi_compare_objects(assertion, a->handle)) {
        vpi_printf("unexpected callback %d\n", (int)reason);
        return 0;
    }
    a->tally[r]++;
    if ((r == R_SUCCESS || r == R_VACUOUS) && a->pass_on) a->pass_expected++;
    if (r == R_FAILURE && a->fail_on) a->fail_expected++;
    if (a->trace) {
        if (info)
            vpi_printf("%u %s %s attempt=%u\n", (unsigned)time->low, a->name, reason_names[r],
                       (unsigned)info->attemptStartTime.low);
        else
            vpi_printf("%u %s %s info=NULL\n", (unsigned)time->low, a->name, reason_names[r]);
    }
    return 0;
}

static PLI_INT32 on_second_start(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                                 p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)reason;
    (void)assertion;
    (void)user_data;
    second_start_calls++;
    vpi_printf("%u a_req second Start record attempt=%u\n", (unsigned)time->low,
               (unsigned)info->attemptStartTime.low);
    return 0;
}

static PLI_INT32 on_control(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                            p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)assertion;
    (void)user_data;
    vpi_printf("%u a_req %s info=%s\n", (unsigned)time->low,
               reason == cbAssertionDisablePassAction ? "DisablePassAction" : "?",
               info ? "set" : "NULL");
    return 0;
}

static PLI_INT32 attach(PLI_BYTE8* data) {
    (void)data;
    vpiHandle iterator = vpi_iterate(vpiAssertion, NULL), handle;
    while (iterator && (handle = vpi_scan(iterator)) && assertion_count < MAX_ASSERTIONS) {
        assertion_t* a = &assertions[assertion_count++];
        a->handle = handle;
        snprintf(a->name, sizeof(a->name), "%s", (const char*)vpi_get_str(vpiFullName, handle));
        a->pass_on = a->fail_on = 1;
        vpi_printf("assertion %s %s\n", a->name, (const char*)vpi_get_str(vpiType, handle));
        for (int r = 0; r < R_COUNT; ++r) {
            if (!vpi_register_assertion_cb(handle, reasons[r], on_attempt, (PLI_BYTE8*)a))
                check("register");
        }
        if (strcmp(a->name, "tb.a_req") == 0) a_req = a;
    }
    a_req->trace = 1;
    second_start = vpi_register_assertion_cb(a_req->handle, cbAssertionStart, on_second_start, NULL);
    vpi_register_assertion_cb(a_req->handle, cbAssertionDisablePassAction, on_control, NULL);
    return 0;
}

static PLI_INT32 ctl(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &value);
    vpi_release_handle(args);
    if (value.value.integer == 1) {
        s_vpi_time start = {vpiSimTime, 0, 35, 0.0};
        PLI_INT32 status = vpi_control(vpiAssertionKill, a_req->handle, &start);
        vpi_printf("38 kill a_req attempt 35 -> %d\n", (int)status);
        vpi_printf("remove second Start record -> %d\n", (int)vpi_remove_cb(second_start));
        /* The callbacks keep their own handle of the assertion. */
        vpi_printf("release a_req handle -> %d\n", (int)vpi_release_handle(a_req->handle));
        a_req->handle = vpi_handle_by_name((PLI_BYTE8*)"tb.a_req", NULL);
    } else {
        a_req->pass_on = 0;
        PLI_INT32 status = vpi_control(vpiAssertionDisablePassAction, a_req->handle);
        vpi_printf("44 disable a_req pass action -> %d\n", (int)status);
    }
    return 0;
}

static int hdl(const char* name) {
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)name, NULL), &value);
    return value.value.integer;
}

static void compare(const char* what, int callbacks, const char* counter) {
    int value = hdl(counter);
    vpi_printf("  %s: callbacks %d, %s %d: %s\n", what, callbacks, counter, value,
               callbacks == value ? "match" : "MISMATCH");
}

static PLI_INT32 report(PLI_BYTE8* data) {
    (void)data;
    for (int i = 0; i < assertion_count; ++i) {
        assertion_t* a = &assertions[i];
        int* t = a->tally;
        vpiHandle h = a->handle;
        int concluded = t[R_SUCCESS] + t[R_VACUOUS] + t[R_FAILURE] + t[R_DISABLED] + t[R_KILL];
        vpi_printf("%s: start=%d success=%d vacuous=%d failure=%d disabled=%d kill=%d\n",
                   a->name, t[R_START], t[R_SUCCESS], t[R_VACUOUS], t[R_FAILURE],
                   t[R_DISABLED], t[R_KILL]);
        vpi_printf("  counters: attempts=%d success=%d vacuous=%d failure=%d disabled=%d "
                   "killed=%d\n",
                   (int)vpi_get(vpiAssertAttemptCovered, h), (int)vpi_get(vpiAssertSuccessCovered, h),
                   (int)vpi_get(vpiAssertVacuousSuccessCovered, h),
                   (int)vpi_get(vpiAssertFailureCovered, h), (int)vpi_get(vpiAssertDisableCovered, h),
                   (int)vpi_get(vpiAssertKillCovered, h));
        vpi_printf("  attempts started and not concluded: %d\n", t[R_START] - concluded);
        if (strcmp(a->name, "tb.a_req") == 0) {
            compare("pass actions", a->pass_expected, "tb.a_pass");
            compare("fail actions", a->fail_expected, "tb.a_fail");
        } else if (strcmp(a->name, "tb.m_two") == 0) {
            compare("pass actions", a->pass_expected, "tb.m_pass");
            compare("fail actions", a->fail_expected, "tb.m_fail");
        } else if (strcmp(a->name, "tb.f_fut") == 0) {
            compare("pass actions", a->pass_expected, "tb.f_pass");
            compare("fail actions", a->fail_expected, "tb.f_fail");
        } else if (strcmp(a->name, "tb.k_cov") == 0) {
            compare("cover hits", t[R_SUCCESS], "tb.cov_hits");
        }
    }
    vpi_printf("second Start record ran %d times\n", second_start_calls);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$attach";
    task.calltf = attach;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$ctl";
    task.calltf = ctl;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$report";
    task.calltf = report;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
