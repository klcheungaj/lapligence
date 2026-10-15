/* Companion of S43-D3_next_sim_time_slot.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static int calls;

static void next_time(PLI_INT32 (*routine)(p_cb_data)) {
    s_vpi_time t;
    s_cb_data cb;
    memset(&t, 0, sizeof(t));
    memset(&cb, 0, sizeof(cb));
    t.type = vpiSimTime;
    cb.reason = cbNextSimTime;
    cb.cb_rtn = routine;
    cb.time = &t;
    vpi_register_cb(&cb);
}

static PLI_INT32 next(p_cb_data data) {
    vpi_printf("cbNextSimTime at %u\n", (unsigned)data->time->low);
    if (++calls == 1) next_time(next);
    return 0;
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    next_time(next);
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
