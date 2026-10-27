/* ADV-017: vpi_control assertion operations and the callbacks they raise
 * (IEEE 1800-2009 39.4.1, 39.4.2, 39.5.1, 39.5.2). Companion of control.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle p, im, df, s_x, top, sub;

static const char* name_of(vpiHandle assertion) {
    const char* name = (const char*)vpi_get_str(vpiName, assertion);
    return name ? name : "?";
}

static const char* reason_name(PLI_INT32 reason) {
    switch (reason) {
        case cbAssertionStart: return "Start";
        case cbAssertionSuccess: return "Success";
        case cbAssertionVacuousSuccess: return "VacuousSuccess";
        case cbAssertionFailure: return "Failure";
        case cbAssertionDisabledEvaluation: return "Disabled";
        case cbAssertionKill: return "Kill";
        case cbAssertionDisable: return "Disable";
        case cbAssertionEnable: return "Enable";
        case cbAssertionReset: return "Reset";
        case cbAssertionDisablePassAction: return "DisablePassAction";
        case cbAssertionEnablePassAction: return "EnablePassAction";
        case cbAssertionDisableFailAction: return "DisableFailAction";
        case cbAssertionEnableFailAction: return "EnableFailAction";
        case cbAssertionDisableVacuousAction: return "DisableVacuousAction";
        case cbAssertionEnableNonvacuousAction: return "EnableNonvacuousAction";
        case cbAssertionSysInitialized: return "SysInitialized";
        case cbAssertionSysOn: return "SysOn";
        case cbAssertionSysOff: return "SysOff";
        case cbAssertionSysKill: return "SysKill";
        case cbAssertionSysEnd: return "SysEnd";
        case cbAssertionSysReset: return "SysReset";
        case cbAssertionSysEnablePassAction: return "SysEnablePassAction";
        case cbAssertionSysEnableFailAction: return "SysEnableFailAction";
        case cbAssertionSysDisablePassAction: return "SysDisablePassAction";
        case cbAssertionSysDisableFailAction: return "SysDisableFailAction";
        case cbAssertionSysEnableNonvacuousAction: return "SysEnableNonvacuousAction";
        case cbAssertionSysDisableVacuousAction: return "SysDisableVacuousAction";
        default: return "?";
    }
}

/* Print the status a routine returned and the error it left (38.2). */
static void status(const char* what, PLI_INT32 value) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("  %s -> %d level=%d code=%s\n", what, (int)value, (int)level,
               level ? (const char*)info.code : "-");
}

/* Attempt tallies of the untraced assertions: start, success, failure, kill. */
static int tally[3][4];

static PLI_INT32 on_attempt(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                            p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    if (user_data) {
        int* t = (int*)user_data;
        t[reason == cbAssertionStart     ? 0
          : reason == cbAssertionSuccess ? 1
          : reason == cbAssertionFailure ? 2
                                         : 3]++;
        return 0;
    }
    if (info)
        vpi_printf("%u %s %s attempt=%u\n", (unsigned)time->low, name_of(assertion),
                   reason_name(reason), (unsigned)info->attemptStartTime.low);
    else
        vpi_printf("%u %s %s info=NULL\n", (unsigned)time->low, name_of(assertion),
                   reason_name(reason));
    return 0;
}

static PLI_INT32 on_system(p_cb_data data) {
    vpi_printf("%u %s obj=%s\n", (unsigned)data->time->low, reason_name(data->reason),
               data->obj ? "set" : "NULL");
    return 0;
}

static vpiHandle system_cb(PLI_INT32 reason) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = reason;
    cb.cb_rtn = on_system;
    cb.time = &time;
    return vpi_register_cb(&cb);
}

static const PLI_INT32 attempt_reasons[] = {cbAssertionStart, cbAssertionSuccess,
                                            cbAssertionFailure, cbAssertionKill};
static const PLI_INT32 control_reasons[] = {
    cbAssertionDisable,           cbAssertionEnable,           cbAssertionReset,
    cbAssertionDisablePassAction, cbAssertionEnablePassAction, cbAssertionDisableFailAction,
    cbAssertionEnableFailAction,  cbAssertionDisableVacuousAction,
    cbAssertionEnableNonvacuousAction};

static PLI_INT32 setup(PLI_BYTE8* data) {
    (void)data;
    top = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    sub = vpi_handle_by_name((PLI_BYTE8*)"tb.u_sub", NULL);
    p = vpi_handle_by_name((PLI_BYTE8*)"tb.p", NULL);
    im = vpi_handle_by_name((PLI_BYTE8*)"tb.im", NULL);
    df = vpi_handle_by_name((PLI_BYTE8*)"tb.df", NULL);
    s_x = vpi_handle_by_name((PLI_BYTE8*)"tb.u_sub.s_x", NULL);
    vpiHandle traced[] = {p, im, df};
    for (int i = 0; i < 4; ++i) {
        vpi_register_assertion_cb(p, attempt_reasons[i], on_attempt, NULL);
        vpi_register_assertion_cb(im, attempt_reasons[i], on_attempt, (PLI_BYTE8*)tally[0]);
        vpi_register_assertion_cb(df, attempt_reasons[i], on_attempt, (PLI_BYTE8*)tally[1]);
        vpi_register_assertion_cb(s_x, attempt_reasons[i], on_attempt, (PLI_BYTE8*)tally[2]);
    }
    for (int a = 0; a < 3; ++a)
        for (size_t i = 0; i < sizeof(control_reasons) / sizeof(control_reasons[0]); ++i)
            vpi_register_assertion_cb(traced[a], control_reasons[i], on_attempt, NULL);
    vpiHandle info_cb = vpi_register_assertion_cb(p, cbAssertionReset, on_attempt, NULL);
    s_cb_data info;
    memset(&info, 0, sizeof(info));
    vpi_get_cb_info(info_cb, &info);
    vpi_printf("cb info: reason=%s obj=%s cb_rtn=%s\n", reason_name(info.reason),
               vpi_compare_objects(info.obj, p) ? "p" : "other", info.cb_rtn ? "set" : "NULL");
    vpi_remove_cb(info_cb);
    /* Registered after the start of the run: it already occurred. */
    vpiHandle late = system_cb(cbAssertionSysInitialized);
    status("late cbAssertionSysInitialized", late != NULL);
    status("step callback", vpi_register_assertion_cb(p, cbAssertionStepSuccess, on_attempt, NULL) != NULL);
    status("assertion callback on a module", vpi_register_assertion_cb(top, cbAssertionStart, on_attempt, NULL) != NULL);
    status("assertion callback without a routine", vpi_register_assertion_cb(p, cbAssertionStart, NULL, NULL) != NULL);
    status("assertion callback with a time reason", vpi_register_assertion_cb(p, cbAfterDelay, on_attempt, NULL) != NULL);
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
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("%u step %d\n", (unsigned)now.low, (int)value.value.integer);
    s_vpi_time at = {vpiSimTime, 0, 0, 0.0};
    switch (value.value.integer) {
        case 1:
            status("disable p", vpi_control(vpiAssertionDisable, p));
            status("disable im", vpi_control(vpiAssertionDisable, im));
            status("disable df", vpi_control(vpiAssertionDisable, df));
            status("disable NULL", vpi_control(vpiAssertionDisable, NULL));
            status("disable a module", vpi_control(vpiAssertionDisable, top));
            break;
        case 2:
            status("enable p", vpi_control(vpiAssertionEnable, p));
            status("enable im", vpi_control(vpiAssertionEnable, im));
            status("enable df", vpi_control(vpiAssertionEnable, df));
            break;
        case 3: {
            at.low = 35;
            status("kill p attempt 35", vpi_control(vpiAssertionKill, p, &at));
            at.low = 7;
            status("kill p attempt 7", vpi_control(vpiAssertionKill, p, &at));
            s_vpi_time suppressed = {vpiSuppressTime, 0, 0, 0.0};
            status("kill with vpiSuppressTime", vpi_control(vpiAssertionKill, p, &suppressed));
            break;
        }
        case 4:
            status("reset p", vpi_control(vpiAssertionReset, p));
            break;
        case 5:
            status("disable p fail action", vpi_control(vpiAssertionDisableFailAction, p));
            status("disable im pass action", vpi_control(vpiAssertionDisablePassAction, im));
            status("disable p vacuous action", vpi_control(vpiAssertionDisableVacuousAction, p));
            break;
        case 6:
            status("enable p fail action", vpi_control(vpiAssertionEnableFailAction, p));
            status("enable im pass action", vpi_control(vpiAssertionEnablePassAction, im));
            status("enable p nonvacuous action", vpi_control(vpiAssertionEnableNonvacuousAction, p));
            break;
        case 7:
            status("system off u_sub", vpi_control(vpiAssertionSysOff, sub));
            break;
        case 8:
            status("system on", vpi_control(vpiAssertionSysOn, NULL));
            status("system kill p", vpi_control(vpiAssertionSysKill, p));
            status("system reset tb", vpi_control(vpiAssertionSysReset, top));
            break;
        case 9:
            status("system disable pass", vpi_control(vpiAssertionSysDisablePassAction, NULL));
            status("system enable pass", vpi_control(vpiAssertionSysEnablePassAction, NULL));
            status("system disable fail", vpi_control(vpiAssertionSysDisableFailAction, NULL));
            status("system enable fail", vpi_control(vpiAssertionSysEnableFailAction, NULL));
            status("system disable vacuous", vpi_control(vpiAssertionSysDisableVacuousAction, NULL));
            status("system enable nonvacuous", vpi_control(vpiAssertionSysEnableNonvacuousAction, NULL));
            break;
        default:
            at.low = 115;
            status("enable step", vpi_control(vpiAssertionEnableStep, p, &at, vpiAssertionClockSteps));
            status("disable step", vpi_control(vpiAssertionDisableStep, p, &at));
            status("system end u_sub", vpi_control(vpiAssertionSysEnd, sub));
            status("system end", vpi_control(vpiAssertionSysEnd, NULL));
            status("enable p after the end", vpi_control(vpiAssertionEnable, p));
            status("register after the end",
                   vpi_register_assertion_cb(p, cbAssertionStart, on_attempt, NULL) != NULL);
            status("system callback after the end", system_cb(cbAssertionSysOn) != NULL);
            break;
    }
    return 0;
}

static void counters(const char* name, vpiHandle h, const int* t) {
    if (t)
        vpi_printf("%s callbacks: start=%d success=%d failure=%d kill=%d\n", name, t[0], t[1],
                   t[2], t[3]);
    vpi_printf("%s counters: attempts=%d success=%d vacuous=%d failure=%d disabled=%d killed=%d\n",
               name, (int)vpi_get(vpiAssertAttemptCovered, h),
               (int)vpi_get(vpiAssertSuccessCovered, h),
               (int)vpi_get(vpiAssertVacuousSuccessCovered, h),
               (int)vpi_get(vpiAssertFailureCovered, h), (int)vpi_get(vpiAssertDisableCovered, h),
               (int)vpi_get(vpiAssertKillCovered, h));
}

static PLI_INT32 report(PLI_BYTE8* data) {
    (void)data;
    counters("p", p, NULL);
    counters("im", im, tally[0]);
    counters("df", df, tally[1]);
    counters("s_x", s_x, tally[2]);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$setup";
    task.calltf = setup;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$ctl";
    task.calltf = ctl;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$report";
    task.calltf = report;
    vpi_register_systf(&task);
    const PLI_INT32 system_reasons[] = {
        cbAssertionSysInitialized,          cbAssertionSysOn,
        cbAssertionSysOff,                  cbAssertionSysKill,
        cbAssertionSysEnd,                  cbAssertionSysReset,
        cbAssertionSysEnablePassAction,     cbAssertionSysEnableFailAction,
        cbAssertionSysDisablePassAction,    cbAssertionSysDisableFailAction,
        cbAssertionSysEnableNonvacuousAction, cbAssertionSysDisableVacuousAction};
    for (size_t i = 0; i < sizeof(system_reasons) / sizeof(system_reasons[0]); ++i)
        system_cb(system_reasons[i]);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
