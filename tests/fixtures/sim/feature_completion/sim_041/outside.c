/* SIM-041 companion for neg_export_outside_chain.sv: a VPI application whose
 * system task calls a DPI-C export with no import call chain. Portable C11. */
#include "vpi_user.h"
#include <stddef.h>

int err_inc(int n);

static PLI_INT32 outside_call(PLI_BYTE8* data)
{
    (void)data;
    err_inc(1);
    return 0;
}

static void outside_register(void)
{
    s_vpi_systf_data task = {vpiSysTask, 0, "$outside", outside_call, NULL, NULL, NULL};
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {outside_register, NULL};
