#include <string.h>

#include "vpi_user.h"

/* Prints "vpi_printf <n>" for $vpi_say(n). */
static PLI_INT32 say_calltf(PLI_BYTE8* user_data) {
    (void)user_data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = args ? vpi_scan(args) : NULL;
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    if (arg) vpi_get_value(arg, &value);
    if (args && arg) vpi_release_handle(args);
    vpi_printf("vpi_printf %d\n", (int)value.value.integer);
    return 0;
}

static void register_say(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$vpi_say";
    data.calltf = say_calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_say, 0};
