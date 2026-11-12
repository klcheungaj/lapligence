/* SIM-043 T1-D11: value-change callbacks on the variables of
 * value_changes.sv print the time and the new value; the time-0
 * initialization is left out. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 changed(p_cb_data data)
{
    PLI_UINT32 time = data->time ? data->time->low : 0u;
    if (time == 0u) return 0;
    if (data->value->format == vpiRealVal)
        vpi_printf("%u vpi %s=%.1f\n", time, data->user_data, data->value->value.real);
    else
        vpi_printf("%u vpi %s=%s\n", time, data->user_data, data->value->value.str);
    return 0;
}

static void watch(const char* name, PLI_INT32 format)
{
    static s_vpi_time time;
    static s_vpi_value values[8];
    static int used;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    time.type = vpiSimTime;
    values[used].format = format;
    data.reason = cbValueChange;
    data.cb_rtn = changed;
    data.obj = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    data.time = &time;
    data.value = &values[used++];
    data.user_data = (PLI_BYTE8*)name;
    vpi_register_cb(&data);
}

static PLI_INT32 started(p_cb_data data)
{
    (void)data;
    watch("tb.g", vpiBinStrVal);
    watch("tb.h", vpiBinStrVal);
    watch("tb.k", vpiBinStrVal);
    watch("tb.v", vpiBinStrVal);
    watch("tb.w", vpiHexStrVal);
    watch("tb.r", vpiRealVal);
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
