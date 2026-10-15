/* Companion of S43-D1_read_write_synch_after_nba.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static int value_of(const char* name) {
    s_vpi_value v;
    v.format = vpiIntVal;
    vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)name, NULL), &v);
    return (int)v.value.integer;
}

static PLI_INT32 nba_synch(p_cb_data data) {
    (void)data;
    vpi_printf("cbNBASynch b=%d\n", value_of("tb.b"));
    return 0;
}

static PLI_INT32 rw_synch(p_cb_data data) {
    (void)data;
    vpi_printf("cbReadWriteSynch b=%d\n", value_of("tb.b"));
    s_vpi_value v;
    v.format = vpiIntVal;
    v.value.integer = 3;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.c", NULL), &v, NULL, vpiNoDelay);
    return 0;
}

static void at(PLI_INT32 reason, PLI_INT32 (*routine)(p_cb_data), unsigned delay) {
    s_vpi_time t;
    s_cb_data cb;
    memset(&t, 0, sizeof(t));
    memset(&cb, 0, sizeof(cb));
    t.type = vpiSimTime;
    t.low = delay;
    cb.reason = reason;
    cb.cb_rtn = routine;
    cb.time = &t;
    vpi_register_cb(&cb);
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    at(cbNBASynch, nba_synch, 1);
    at(cbReadWriteSynch, rw_synch, 1);
    return 0;
}

static void startup(void) {
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbStartOfSimulation;
    cb.cb_rtn = start;
    vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
