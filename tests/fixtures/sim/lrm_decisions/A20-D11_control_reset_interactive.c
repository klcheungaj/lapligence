/* Companion of A20-D11_control_reset_interactive.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 d11(PLI_BYTE8* data) {
    (void)data;
    PLI_INT32 result = vpi_control(vpiReset, 0, 0, 0);
    vpi_printf("vpiReset: %d level=%d\n", (int)result, (int)vpi_chk_error(NULL));
    result = vpi_control(vpiSetInteractiveScope, vpi_handle_by_name((PLI_BYTE8*)"tb", NULL));
    vpi_printf("vpiSetInteractiveScope: %d level=%d\n", (int)result, (int)vpi_chk_error(NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d11";
    task.calltf = d11;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
