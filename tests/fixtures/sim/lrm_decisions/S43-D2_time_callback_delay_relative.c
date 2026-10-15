/* Companion of S43-D2_time_callback_delay_relative.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 report(p_cb_data data) {
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("%s at %u\n", data->user_data, (unsigned)now.low);
    return 0;
}

static vpiHandle at(PLI_INT32 reason, PLI_INT32 type, unsigned delay, double real,
                    const char* label) {
    s_vpi_time t;
    s_cb_data cb;
    memset(&t, 0, sizeof(t));
    memset(&cb, 0, sizeof(cb));
    t.type = type;
    t.low = delay;
    t.real = real;
    cb.reason = reason;
    cb.cb_rtn = report;
    cb.time = &t;
    cb.obj = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    cb.user_data = (PLI_BYTE8*)label;
    return vpi_register_cb(&cb);
}

static PLI_INT32 d2(PLI_BYTE8* user) {
    (void)user;
    at(cbAfterDelay, vpiSimTime, 2, 0.0, "cbAfterDelay 2");
    at(cbReadOnlySynch, vpiScaledRealTime, 0, 4.0, "cbReadOnlySynch 4.0");
    vpiHandle zero = at(cbAfterDelay, vpiSimTime, 0, 0.0, "cbAfterDelay 0");
    vpi_printf("zero-delay cbAfterDelay at 3: %s, error %d\n", zero ? "accepted" : "refused",
               (int)vpi_chk_error(NULL));
    return 0;
}

static void startup(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof(tf));
    tf.type = vpiSysTask;
    tf.tfname = (PLI_BYTE8*)"$d2";
    tf.calltf = d2;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
