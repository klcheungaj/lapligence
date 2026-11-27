/* Companion of CS-D4_vpi_release_keeps_action_block_writes.sv: a VPI
 * application (vlog_startup_routines). $d4a(1) forces tb.passes, tb.fails,
 * tb.init_v and tb.y to 100, 90, 80 and 70 through vpi_put_value; $d4a(0)
 * releases them. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 d4a(PLI_BYTE8* data)
{
    static const char* names[] = {"tb.passes", "tb.fails", "tb.init_v", "tb.y"};
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    s_vpi_value value;
    int force;
    (void)data;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(arguments), &value);
    vpi_free_object(arguments);
    force = value.value.integer;
    for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); ++i) {
        vpiHandle object = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        value.format = vpiIntVal;
        value.value.integer = 100 - 10 * (PLI_INT32)i;
        vpi_put_value(object, &value, NULL, force ? vpiForceFlag : vpiReleaseFlag);
    }
    return 0;
}

static void startup(void)
{
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d4a";
    task.calltf = d4a;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
