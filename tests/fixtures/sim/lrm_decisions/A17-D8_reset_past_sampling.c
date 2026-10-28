/* Decision A17-D8 (see A17-D8_reset_past_sampling.sv). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 d8(PLI_BYTE8* data) {
    (void)data;
    vpiHandle iterator = vpi_iterate(vpiAssertion, NULL), assertion;
    while ((assertion = vpi_scan(iterator))) {
        const char* name = vpi_get_str(vpiName, assertion);
        if (strcmp(name, "p1") && strcmp(name, "r1")) continue;
        vpi_printf("reset %s -> %d\n", name, (int)vpi_control(vpiAssertionReset, assertion));
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d8";
    task.calltf = d8;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
