/* SIM-099 annex_override.sv companion: VPI applications for two built-in
 * names llg has no lowering for (IEEE 1800-2009 36.3.2). Portable C11. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 reset_count_calltf(PLI_BYTE8* user_data)
{
    s_vpi_value value;
    (void)user_data;
    value.format = vpiIntVal;
    value.value.integer = 2;
    vpi_put_value(vpi_handle(vpiSysTfCall, NULL), &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 list_calltf(PLI_BYTE8* user_data)
{
    (void)user_data;
    vpi_printf("list from the VPI application\n");
    return 0;
}

static void startup(void)
{
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysFunc;
    data.sysfunctype = vpiIntFunc;
    data.tfname = (PLI_BYTE8*)"$reset_count";
    data.calltf = reset_count_calltf;
    vpi_register_systf(&data);
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$list";
    data.calltf = list_calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
