/* Companion of S43-D10_control_from_start_of_simulation.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static int ends;

static PLI_INT32 end(p_cb_data data) {
    (void)data;
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("cbEndOfSimulation %d at %u\n", ++ends, (unsigned)now.low);
    return 0;
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    vpi_printf("cbStartOfSimulation\n");
    /* The last statement: what follows a vpiFinish request is not part of
     * the decision. */
    vpi_control(vpiFinish, 0);
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
