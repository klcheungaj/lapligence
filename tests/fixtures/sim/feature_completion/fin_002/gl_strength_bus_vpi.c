/* FIN-002 VPI application of gl_strength_bus.sv: $strength prints the
 * strength value (logic and both strength components) of the scalar net
 * its string literal argument names. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 strength(PLI_BYTE8* user)
{
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle path = arguments ? vpi_scan(arguments) : NULL;
    vpiHandle object = NULL;
    s_vpi_value value;
    (void)user;
    if (path) {
        value.format = vpiStringVal;
        vpi_get_value(path, &value);
        object = vpi_handle_by_name(value.value.str, NULL);
        vpi_free_object(arguments);
    }
    if (!object) {
        vpi_printf("  vpi: object missing\n");
        return 0;
    }
    value.format = vpiStrengthVal;
    vpi_get_value(object, &value);
    vpi_printf("  vpi %s: logic=%d s0=%d s1=%d\n", vpi_get_str(vpiName, object),
               (int)value.value.strength->logic, (int)value.value.strength->s0,
               (int)value.value.strength->s1);
    return 0;
}

static void startup(void)
{
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$strength";
    data.calltf = strength;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
