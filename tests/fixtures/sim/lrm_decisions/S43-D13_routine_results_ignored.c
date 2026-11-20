/* Companion of S43-D13_routine_results_ignored.sv: a VPI application whose
 * routines all return 1. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 delayed(p_cb_data data) {
    s_vpi_time now;
    (void)data;
    memset(&now, 0, sizeof(now));
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("cbAfterDelay at %u\n", (unsigned)now.low);
    return 1;
}

static PLI_INT32 started(p_cb_data data) {
    (void)data;
    vpi_printf("cbStartOfSimulation\n");
    return 1;
}

static PLI_INT32 ret1_compiletf(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("compiletf\n");
    return 1;
}

static PLI_INT32 ret1_calltf(PLI_BYTE8* data) {
    s_vpi_value value;
    s_vpi_time delay;
    s_cb_data cb;
    (void)data;
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    vpi_put_value(NULL, &value, NULL, vpiNoDelay);
    vpi_printf("calltf: error level %d\n", (int)vpi_chk_error(NULL));
    memset(&delay, 0, sizeof(delay));
    delay.type = vpiSimTime;
    delay.low = 1;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbAfterDelay;
    cb.cb_rtn = delayed;
    cb.time = &delay;
    vpi_register_cb(&cb);
    return 1;
}

static void startup(void) {
    s_vpi_systf_data systf;
    s_cb_data cb;
    memset(&systf, 0, sizeof(systf));
    systf.type = vpiSysTask;
    systf.tfname = (PLI_BYTE8*)"$ret1";
    systf.compiletf = ret1_compiletf;
    systf.calltf = ret1_calltf;
    vpi_register_systf(&systf);
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbStartOfSimulation;
    cb.cb_rtn = started;
    vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
