/* SIM-043: object lookup with and without the model's VPI catalog. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 later(p_cb_data data) {
    vpi_printf("[%u] after delay\n", (unsigned)data->time->low);
    return 0;
}

static PLI_INT32 started(p_cb_data data) {
    (void)data;
    vpiHandle v = vpi_handle_by_name((PLI_BYTE8*)"tb.v", NULL);
    vpi_printf("tb.v %s\n", v ? "found" : "missing");
    s_vpi_time time;
    s_cb_data delay;
    memset(&delay, 0, sizeof(delay));
    time.type = vpiSimTime;
    time.high = 0;
    time.low = 2;
    delay.reason = cbAfterDelay;
    delay.cb_rtn = later;
    delay.time = &time;
    vpi_register_cb(&delay);
    return 0;
}

static void startup(void) {
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = cbStartOfSimulation;
    data.cb_rtn = started;
    vpi_register_cb(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
