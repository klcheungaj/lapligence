/* ADV-026 notify.sv companion: value-change callbacks on the continuously
 * assigned copies of the two memories. A callback reports only while the
 * design's `armed` variable is set, so the time-0 initialization is quiet.
 * Portable C11. */
#include "vpi_user.h"
#include <string.h>

static int armed(void)
{
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)"tb.armed", NULL), &value);
    return value.value.integer == 1;
}

static PLI_INT32 changed(p_cb_data data)
{
    if (!armed()) return 0;
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
    watch("tb.sumf");
    watch("tb.sums");
    watch("tb.totf");
    watch("tb.tots");
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
