/* FIN-002 VPI application of tb_fifo_env.sv. Its value-change callback on
 * the FIFO fill count writes `stall` with an inertial delay when the FIFO
 * becomes full and registers a timed callback that clears it. */
#include "vpi_user.h"
#include <string.h>

#define STALL_DELAY 2
#define STALL_LENGTH 25

static vpiHandle stall;
static int fills;
static int clears;
static int changes;

static void write_stall(int level, int delay)
{
    s_vpi_value value;
    s_vpi_time time;
    value.format = vpiIntVal;
    value.value.integer = level;
    time.type = vpiSimTime;
    time.high = 0;
    time.low = (PLI_UINT32)delay;
    vpi_put_value(stall, &value, &time, delay ? vpiInertialDelay : vpiNoDelay);
}

static PLI_INT32 clear_stall(p_cb_data data)
{
    clears++;
    write_stall(0, 0);
    vpi_printf("[%u] vpi: stall cleared\n", data->time->low);
    return 0;
}

static PLI_INT32 count_changed(p_cb_data data)
{
    static s_vpi_time time;
    s_cb_data later;
    changes++;
    if (data->value->value.integer != 4) return 0;
    fills++;
    write_stall(1, STALL_DELAY);
    memset(&later, 0, sizeof(later));
    time.type = vpiSimTime;
    time.high = 0;
    time.low = STALL_LENGTH;
    later.reason = cbAfterDelay;
    later.cb_rtn = clear_stall;
    later.time = &time;
    vpi_register_cb(&later);
    vpi_printf("[%u] vpi: full, stall requested\n", data->time->low);
    return 0;
}

static PLI_INT32 started(p_cb_data data)
{
    static s_vpi_time time;
    static s_vpi_value value;
    s_cb_data watch;
    (void)data;
    stall = vpi_handle_by_name((PLI_BYTE8*)"tb.bus.stall", NULL);
    memset(&watch, 0, sizeof(watch));
    time.type = vpiSimTime;
    value.format = vpiIntVal;
    watch.reason = cbValueChange;
    watch.cb_rtn = count_changed;
    watch.obj = vpi_handle_by_name((PLI_BYTE8*)"tb.dut.count", NULL);
    watch.time = &time;
    watch.value = &value;
    if (stall && watch.obj) vpi_register_cb(&watch);
    else vpi_printf("vpi: objects missing\n");
    return 0;
}

static PLI_INT32 ended(p_cb_data data)
{
    (void)data;
    vpi_printf("vpi: count changes=%d fills=%d clears=%d\n", changes, fills, clears);
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
