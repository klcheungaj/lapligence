/* SIM-042: delayed puts as the only future events (decision S42-D5). The
 * transport put 4@5 is the only event put_alive schedules; the value-change
 * callback its write runs then schedules 6@7 with an inertial delay, which
 * removes every pending event on the object while the 4@5 write is still
 * in progress. Companion of put_alive.sv. */
#include "vpi_user.h"
#include <string.h>

static void put(PLI_INT32 integer, unsigned delay, PLI_INT32 flags) {
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiIntVal;
    v.value.integer = integer;
    s_vpi_time t;
    memset(&t, 0, sizeof(t));
    t.type = vpiSimTime;
    t.low = delay;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.d", NULL), &v, &t, flags);
    if (vpi_chk_error(NULL)) vpi_printf("FAIL put %d\n", (int)integer);
}

static PLI_INT32 changed(p_cb_data data) {
    if (data->value->value.integer == 4) put(6, 2, vpiInertialDelay);
    return 0;
}

static PLI_INT32 put_alive(PLI_BYTE8* data) {
    (void)data;
    static s_vpi_time time = {vpiSuppressTime, 0, 0, 0.0};
    static s_vpi_value value = {vpiIntVal, {0}};
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbValueChange;
    cb.cb_rtn = changed;
    cb.obj = vpi_handle_by_name((PLI_BYTE8*)"tb.d", NULL);
    cb.time = &time;
    cb.value = &value;
    vpi_register_cb(&cb);
    put(4, 5, vpiTransportDelay);
    return 0;
}

static void register_all(void) {
    s_vpi_systf_data t;
    memset(&t, 0, sizeof(t));
    t.type = vpiSysTask;
    t.tfname = (PLI_BYTE8*)"$put_alive";
    t.calltf = put_alive;
    vpi_register_systf(&t);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
