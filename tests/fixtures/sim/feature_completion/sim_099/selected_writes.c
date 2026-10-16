/* SIM-099 selected_writes.sv companion: value-change callbacks on the
 * continuously assigned copies tb.seen and tb.low. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 changed(p_cb_data data)
{
    vpi_printf("[%u] vpi %s=%s\n", data->time ? data->time->low : 0u, data->user_data,
               data->value->value.str);
    return 0;
}

static void watch(const char* name)
{
    static s_vpi_time time;
    static s_vpi_value value;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    time.type = vpiSimTime;
    value.format = vpiHexStrVal;
    data.reason = cbValueChange;
    data.cb_rtn = changed;
    data.obj = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    data.time = &time;
    data.value = &value;
    data.user_data = (PLI_BYTE8*)name;
    vpi_register_cb(&data);
}

static PLI_INT32 started(p_cb_data data)
{
    (void)data;
    watch("tb.seen");
    watch("tb.low");
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
