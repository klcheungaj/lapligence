/* Companion of S43-D8_control_finish_after_callback.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static int ends;

static PLI_INT32 finish(p_cb_data data) {
    PLI_INT32 status = vpi_control(vpiFinish, 0);
    vpi_printf("vpiFinish at %u -> %d\n", (unsigned)data->time->low, (int)status);
    return 0;
}

static PLI_INT32 end(p_cb_data data) {
    vpi_printf("cbEndOfSimulation %d at %u\n", ++ends, (unsigned)data->time->low);
    return 0;
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    PLI_INT32 status = vpi_control(vpiStop, 0);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("vpiStop before the run -> %d, error %d\n", (int)status, (int)level);
    status = vpi_control(vpiFinish, 7);
    level = vpi_chk_error(NULL);
    vpi_printf("vpiFinish level 7 -> %d, error %d\n", (int)status, (int)level);
    status = vpi_control(vpiReset, 0, 0, 0);
    level = vpi_chk_error(NULL);
    vpi_printf("vpiReset -> %d, error %d\n", (int)status, (int)level);
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
