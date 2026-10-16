/* SIM-099 VPI application of testbench.sv: a value-change callback on the
 * ALU result and an end-of-simulation summary. */
#include "vpi_user.h"
#include <string.h>

static int changes;

static PLI_INT32 result_changed(p_cb_data data)
{
    changes++;
    vpi_printf("[%u] vpi result=%d\n", data->time ? data->time->low : 0u,
               (int)data->value->value.integer);
    return 0;
}

static PLI_INT32 started(p_cb_data data)
{
    static s_vpi_time time;
    static s_vpi_value value;
    s_cb_data watch;
    (void)data;
    memset(&watch, 0, sizeof(watch));
    time.type = vpiSimTime;
    value.format = vpiIntVal;
    watch.reason = cbValueChange;
    watch.cb_rtn = result_changed;
    watch.obj = vpi_handle_by_name((PLI_BYTE8*)"tb.dut.result", NULL);
    watch.time = &time;
    watch.value = &value;
    vpi_printf("vpi watch %s\n", watch.obj ? "tb.dut.result" : "missing");
    if (watch.obj) vpi_register_cb(&watch);
    return 0;
}

static PLI_INT32 ended(p_cb_data data)
{
    (void)data;
    vpi_printf("vpi end: %d result changes\n", changes);
    return 0;
}

static void action(PLI_INT32 reason, PLI_INT32 (*routine)(p_cb_data))
{
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = reason;
    data.cb_rtn = routine;
    vpi_register_cb(&data);
}

static void startup(void)
{
    action(cbStartOfSimulation, started);
    action(cbEndOfSimulation, ended);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
