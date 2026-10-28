/* Decision A17-D5: callbacks on sequence and property instances. See
 * A17-D5_instance_callbacks.sv. */
#include "sv_vpi_user.h"
#include <string.h>

static PLI_INT32 seen(PLI_INT32 reason, p_vpi_time time, vpiHandle h, p_vpi_attempt_info info,
                      PLI_BYTE8* user_data) {
    const char* name = reason == cbAssertionStart     ? "start"
                       : reason == cbAssertionSuccess ? "success"
                                                      : "failure";
    vpi_printf("%u %s %s %s attempt=%u\n", (unsigned)time->low, (const char*)user_data, name,
               (const char*)vpi_get_str(vpiType, h), (unsigned)info->attemptStartTime.low);
    return 0;
}

static void result(const char* what, int ok) {
    s_vpi_error_info error;
    memset(&error, 0, sizeof(error));
    int level = vpi_chk_error(&error);
    vpi_printf("%s -> %s%s%s\n", what, ok ? "ok" : "refused", level ? " " : "",
               level ? (const char*)error.code : "");
}

static void watch(vpiHandle h, PLI_BYTE8* label) {
    static const int reasons[] = {cbAssertionStart, cbAssertionSuccess, cbAssertionFailure};
    for (int i = 0; i < 3; ++i) vpi_register_assertion_cb(h, reasons[i], seen, label);
}

static PLI_INT32 d5(PLI_BYTE8* data) {
    (void)data;
    vpiHandle p = vpi_handle(vpiProperty, vpi_handle_by_name((PLI_BYTE8*)"tb.r1", NULL));
    vpiHandle spec = vpi_handle(vpiProperty, vpi_handle_by_name((PLI_BYTE8*)"tb.r2", NULL));
    vpiHandle s = vpi_handle(vpiPropertyExpr, spec);
    vpiHandle q = vpi_handle(vpiProperty, vpi_handle_by_name((PLI_BYTE8*)"tb.r3", NULL));
    vpiHandle arguments = vpi_iterate(vpiArgument, q);
    vpiHandle nested = vpi_scan(arguments);
    vpi_release_handle(arguments);
    watch(p, (PLI_BYTE8*)"p");
    watch(s, (PLI_BYTE8*)"s");
    result("p vacuous success",
           vpi_register_assertion_cb(p, cbAssertionVacuousSuccess, seen, NULL) != NULL);
    result("p disable", vpi_register_assertion_cb(p, cbAssertionDisable, seen, NULL) != NULL);
    result("nested start", vpi_register_assertion_cb(nested, cbAssertionStart, seen, NULL) != NULL);
    result("control p", vpi_control(vpiAssertionDisable, p));
    return 0;
}

static void startup(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof(tf));
    tf.type = vpiSysTask;
    tf.tfname = "$d5";
    tf.calltf = d5;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
