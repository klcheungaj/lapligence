/* Companion of CS-D4_vpi_release_constant_parts.sv: a VPI application
 * (vlog_startup_routines). $d4c(1) forces and $d4c(0) releases every
 * variable the case lists, through vpi_put_value: packed ones to 8'hAA,
 * the real one to 7.25. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 d4c(PLI_BYTE8* data)
{
    static const char* names[] = {"tb.v", "tb.b", "tb.c", "tb.m", "tb.n", "tb.s",
                                  "tb.a[0]", "tb.a[1]", "tb.g", "tb.h", "tb.u",
                                  "tb.e[1][2]", "tb.t", "tb.x", "tb.y", "tb.r"};
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
        if (vpi_get(vpiType, object) == vpiRealVar) {
            value.format = vpiRealVal;
            value.value.real = 7.25;
        } else {
            value.format = vpiIntVal;
            value.value.integer = 0xAA;
        }
        vpi_put_value(object, &value, NULL, force ? vpiForceFlag : vpiReleaseFlag);
    }
    return 0;
}

static void startup(void)
{
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d4c";
    task.calltf = d4c;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
