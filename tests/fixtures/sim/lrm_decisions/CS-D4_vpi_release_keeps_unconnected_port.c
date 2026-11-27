/* Companion of CS-D4_vpi_release_keeps_unconnected_port.sv: a VPI
 * application (vlog_startup_routines). $d4u(1) forces tb.c.u to 70 through
 * vpi_put_value; $d4u(0) releases it. */
#include "vpi_user.h"
#include <string.h>
static PLI_INT32 d4u_calltf(PLI_BYTE8* data)
{
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    s_vpi_value value;
    int force;
    (void)data;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(arguments), &value);
    vpi_free_object(arguments);
    force = value.value.integer;
    static const char* names[] = {"tb.c.u"};
    for (int i = 0; i < 1; ++i) {
        vpiHandle object = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        value.format = vpiIntVal;
        value.value.integer = 70;
        vpi_put_value(object, &value, NULL, force ? vpiForceFlag : vpiReleaseFlag);
    }
    return 0;
}
static void startup(void)
{
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d4u";
    task.calltf = d4u_calltf;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, NULL};
