/* SIM-099 gc_roots.sv companion: a DPI-C context import that allocates
 * through an exported function, and a VPI application answering each change
 * of tb.req with tb.ack = req + 1 one time unit later. Portable C11. */
#include "svdpi.h"
#include "vpi_user.h"
#include <string.h>

int sv_alloc(int k);

DPI_DLLESPEC int c_churn(int n)
{
    int made = 0;
    for (int k = 0; k < n; k++) made += sv_alloc(k) == k;
    return made;
}

static vpiHandle ack;

static PLI_INT32 req_changed(p_cb_data data)
{
    s_vpi_value value;
    s_vpi_time delay;
    value.format = vpiIntVal;
    value.value.integer = data->value->value.integer + 1;
    delay.type = vpiSimTime;
    delay.high = 0;
    delay.low = 1;
    vpi_put_value(ack, &value, &delay, vpiInertialDelay);
    return 0;
}

static PLI_INT32 started(p_cb_data data)
{
    static s_vpi_time time;
    static s_vpi_value value;
    s_cb_data watch;
    (void)data;
    ack = vpi_handle_by_name((PLI_BYTE8*)"tb.ack", NULL);
    memset(&watch, 0, sizeof(watch));
    time.type = vpiSimTime;
    value.format = vpiIntVal;
    watch.reason = cbValueChange;
    watch.cb_rtn = req_changed;
    watch.obj = vpi_handle_by_name((PLI_BYTE8*)"tb.req", NULL);
    watch.time = &time;
    watch.value = &value;
    vpi_register_cb(&watch);
    return 0;
}

static void startup(void)
{
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = cbStartOfSimulation;
    data.cb_rtn = started;
    vpi_register_cb(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
