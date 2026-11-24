/* Companion of CS-D4_vpi_release_reestablishes_assignment.sv: a VPI
 * application (vlog_startup_routines). $d4(1) forces and $d4(0) releases
 * the five variables. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 d4(PLI_BYTE8* data)
{
    static const char* names[] = {"tb.ported", "tb.assigned", "tb.stored", "tb.tied", "tb.delayed"};
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    s_vpi_value value;
    int force;
    (void)data;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(arguments), &value);
    vpi_free_object(arguments);
    force = value.value.integer;
    for (int i = 0; i < 5; ++i) {
        vpiHandle object = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        value.format = vpiIntVal;
        value.value.integer = 90 + i;
        vpi_put_value(object, &value, NULL, force ? vpiForceFlag : vpiReleaseFlag);
    }
    return 0;
}

static void startup(void)
{
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d4";
    task.calltf = d4;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
