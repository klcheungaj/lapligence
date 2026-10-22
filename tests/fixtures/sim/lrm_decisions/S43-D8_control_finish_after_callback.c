/* Companion of S43-D8_control_finish_after_callback.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static int ends;

static PLI_INT32 finish(p_cb_data data) {
    /* Level 7 is outside the $finish levels 0-2; it is still honoured. */
    PLI_INT32 status = vpi_control(vpiFinish, 7);
    vpi_printf("vpiFinish at %u -> %d\n", (unsigned)data->time->low, (int)status);
    return 0;
}

static PLI_INT32 end(p_cb_data data) {
    /* An action callback need not carry a time (38.36.3); ask for it. */
    (void)data;
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("cbEndOfSimulation %d at %u\n", ++ends, (unsigned)now.low);
    return 0;
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    s_vpi_time t;
    s_cb_data cb;
    memset(&t, 0, sizeof(t));
    memset(&cb, 0, sizeof(cb));
    t.type = vpiSimTime;
    t.low = 2;
    cb.reason = cbReadWriteSynch;
    cb.cb_rtn = finish;
    cb.time = &t;
    vpi_register_cb(&cb);
    return 0;
}

static void startup(void) {
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbStartOfSimulation;
    cb.cb_rtn = start;
    vpi_register_cb(&cb);
    cb.reason = cbEndOfSimulation;
    cb.cb_rtn = end;
    vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
