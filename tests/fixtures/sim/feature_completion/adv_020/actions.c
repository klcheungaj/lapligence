/* ADV-020: cbError, cbPLIError and cbSignal (IEEE 1800-2009 38.36.3), and
 * the refusal of the reasons llg does not raise. Companion of actions.sv.
 * Every registration and callback prints the vpi_chk_error() status. */
#include "sv_vpi_user.h"
#include <signal.h>
#include <stdio.h>
#include <string.h>

static vpiHandle error_cb, pli_cb, signal_cb;

static void report(const char* label) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("%s: level=%d code=%s\n", label, (int)level,
               level && info.code ? (const char*)info.code : "-");
}

static PLI_INT32 now(void) {
    s_vpi_time time;
    time.type = vpiSimTime;
    vpi_get_time(NULL, &time);
    return (PLI_INT32)time.low;
}

static PLI_INT32 on_action(p_cb_data data) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    const char* message = level && info.message ? (const char*)info.message : "";
    const char* name = data->reason == cbError ? "cbError" : data->reason == cbPLIError
                                                                 ? "cbPLIError"
                                                                 : "cbSignal";
    vpi_printf("  %s(%s) at %d: obj=%s index=%s level=%d state=%d code=%s", name,
               (const char*)data->user_data, (int)now(), data->obj ? "set" : "NULL",
               data->reason == cbSignal ? (data->index == SIGTERM ? "SIGTERM" : "other")
                                        : (data->index ? "nonzero" : "0"),
               (int)level, level ? (int)info.state : 0,
               level && info.code ? (const char*)info.code : "-");
    /* The message names the report; print the part that is the same on
     * every host. */
    const char* words[] = {"first error 0", "assertion assert failed", NULL};
    for (int i = 0; words[i]; ++i)
        if (strstr(message, words[i])) vpi_printf(" message has \"%s\"", words[i]);
    vpi_printf("\n");
    if (data->reason == cbPLIError) {
        /* An error raised inside a cbPLIError routine is not reported
         * again. */
        (void)vpi_get(vpiSize, NULL);
    }
    return 0;
}

static vpiHandle add(const char* label, PLI_INT32 reason, PLI_INT32 index) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = reason;
    cb.cb_rtn = on_action;
    cb.time = &time;
    cb.index = index;
    cb.user_data = (PLI_BYTE8*)label;
    vpiHandle handle = vpi_register_cb(&cb);
    char text[96];
    snprintf(text, sizeof(text), "register %s %s", label, handle ? "handle" : "NULL");
    report(text);
    return handle;
}

static PLI_INT32 act_register(PLI_BYTE8* data) {
    (void)data;
    error_cb = add("error", cbError, 0);
    signal_cb = add("sigterm", cbSignal, SIGTERM);
    add("signal 0", cbSignal, 0);
    add("signal -1", cbSignal, -1);
    static const struct {
        const char* label;
        PLI_INT32 reason;
    } refused[] = {
        {"cbStmt", cbStmt},
        {"cbAssign", cbAssign},
        {"cbDeassign", cbDeassign},
        {"cbUnresolvedSystf", cbUnresolvedSystf},
        {"cbStartOfSave", cbStartOfSave},
        {"cbEndOfRestart", cbEndOfRestart},
        {"cbStartOfReset", cbStartOfReset},
        {"cbEnterInteractive", cbEnterInteractive},
        {"cbInteractiveScopeChange", cbInteractiveScopeChange},
        {"reason 999", 999},
    };
    for (size_t i = 0; i < sizeof(refused) / sizeof(refused[0]); ++i)
        add(refused[i].label, refused[i].reason, 0);
    /* Registered last, so the refusals above are not reported to it. */
    pli_cb = add("pli", cbPLIError, 0);
    return 0;
}

/* Two failing routines: each is reported to cbPLIError at the next routine
 * call (after the failing routine returned) or when this task returns. */
static PLI_INT32 act_pli(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("before the failing call\n");
    PLI_INT32 size = vpi_get(vpiSize, NULL);
    report("vpi_get(vpiSize, NULL)");
    vpi_printf("size=%d, next call\n", (int)size);
    (void)vpi_get(vpiSize, NULL);
    vpi_printf("returning\n");
    return 0;
}

/* The signal is caught; the records run at the next routine call. */
static PLI_INT32 act_signal(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("raising SIGTERM\n");
    raise(SIGTERM);
    vpi_printf("after raise\n");
    return 0;
}

static PLI_INT32 act_remove(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("remove error=%d pli=%d sigterm=%d\n", (int)vpi_remove_cb(error_cb),
               (int)vpi_remove_cb(pli_cb), (int)vpi_remove_cb(signal_cb));
    (void)vpi_get(vpiSize, NULL);
    vpi_printf("after an unobserved routine error\n");
    return 0;
}

static void reg(const char* name, PLI_INT32 (*calltf)(PLI_BYTE8*)) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof(tf));
    tf.type = vpiSysTask;
    tf.tfname = (PLI_BYTE8*)name;
    tf.calltf = calltf;
    vpi_register_systf(&tf);
}

static void startup(void) {
    reg("$act_register", act_register);
    reg("$act_pli", act_pli);
    reg("$act_signal", act_signal);
    reg("$act_remove", act_remove);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
