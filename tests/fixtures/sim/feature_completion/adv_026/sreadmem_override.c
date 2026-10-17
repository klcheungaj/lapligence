/* ADV-026 sreadmem_override companion: a VPI application registered under the
 * name of the built-in $sreadmemh, which IEEE 1800-2009 36.3.2 would let it
 * override. llg runs $sreadmemh natively (decision S99-D6), so a design that
 * calls it stops at startup; one that calls only $sreadmemb is unaffected.
 * Portable C11. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 sreadmemh_calltf(PLI_BYTE8* user_data)
{
    (void)user_data;
    vpi_printf("sreadmemh from the VPI application\n");
    return 0;
}

static void startup(void)
{
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$sreadmemh";
    data.calltf = sreadmemh_calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
