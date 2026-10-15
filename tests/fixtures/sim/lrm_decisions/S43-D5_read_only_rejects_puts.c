/* Companion of S43-D5_read_only_rejects_puts.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static vpiHandle at(PLI_INT32 reason, unsigned delay, PLI_INT32 (*routine)(p_cb_data)) {
    s_vpi_time t;
    s_cb_data cb;
    memset(&t, 0, sizeof(t));
    memset(&cb, 0, sizeof(cb));
    t.type = vpiSimTime;
    t.low = delay;
    cb.reason = reason;
    cb.cb_rtn = routine;
    cb.time = &t;
    return vpi_register_cb(&cb);
}

static PLI_INT32 read_write(p_cb_data data) {
    s_vpi_value v;
    v.format = vpiIntVal;
    v.value.integer = 7;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.d", NULL), &v, NULL, vpiNoDelay);
    vpi_printf("read-write at %u: put error %d\n", (unsigned)data->time->low,
               (int)vpi_chk_error(NULL));
    return 0;
}

static PLI_INT32 read_only(p_cb_data data) {
    vpiHandle d = vpi_handle_by_name((PLI_BYTE8*)"tb.d", NULL);
    s_vpi_value v;
    s_vpi_time later;
    memset(&later, 0, sizeof(later));
    later.type = vpiSimTime;
    later.low = 3;
    v.format = vpiIntVal;
    v.value.integer = 5;
    vpi_put_value(d, &v, NULL, vpiNoDelay);
    vpi_printf("no-delay put error %d\n", (int)vpi_chk_error(NULL));
    vpi_put_value(d, &v, &later, vpiTransportDelay);
    vpi_printf("delayed put error %d\n", (int)vpi_chk_error(NULL));
    vpi_put_value(d, &v, NULL, vpiForceFlag);
    vpi_printf("force error %d\n", (int)vpi_chk_error(NULL));
    vpiHandle zero = at(cbReadWriteSynch, 0, read_write);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("zero-delay cbReadWriteSynch: %s, error %d\n", zero ? "handle" : "NULL", (int)level);
    vpiHandle later_rw = at(cbReadWriteSynch, 2, read_write);
    vpi_printf("cbReadWriteSynch after 2: %s\n", later_rw ? "handle" : "NULL");
    v.format = vpiIntVal;
    vpi_get_value(d, &v);
    vpi_printf("read-only at %u: d=%d\n", (unsigned)data->time->low, (int)v.value.integer);
    return 0;
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    at(cbReadOnlySynch, 4, read_only);
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
