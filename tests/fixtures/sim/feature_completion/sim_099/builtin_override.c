/* SIM-099 builtin_override companion: a VPI application registered under
 * the name of the built-in $random, the example of IEEE 1800-2009 36.3.2.
 * llg runs $random natively and cannot let the registration override it.
 * Portable C11. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 random_calltf(PLI_BYTE8* user_data)
{
    s_vpi_value value;
    (void)user_data;
    value.format = vpiIntVal;
    value.value.integer = 4;
    vpi_put_value(vpi_handle(vpiSysTfCall, NULL), &value, NULL, vpiNoDelay);
    return 0;
}

static void startup(void)
{
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysFunc;
    data.sysfunctype = vpiIntFunc;
    data.tfname = (PLI_BYTE8*)"$random";
    data.calltf = random_calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
