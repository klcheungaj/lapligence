/* VPI application of arguments.sv. */
#include "sv_vpi_user.h"
#include <string.h>

static PLI_INT32 show_arguments(PLI_BYTE8* user) {
    vpiHandle iterator = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    vpiHandle argument;
    (void)user;
    while (iterator && (argument = vpi_scan(iterator))) {
        s_vpi_value value;
        memset(&value, 0, sizeof(value));
        value.format = vpiStringVal;
        vpi_get_value(argument, &value);
        vpi_printf("type=%d const=%d <%s>", vpi_get(vpiType, argument),
                   vpi_get(vpiConstType, argument), value.value.str);
        if (vpi_get(vpiType, argument) == vpiParameter) {
            vpi_printf(" %s", vpi_get_str(vpiFullName, argument));
        }
        vpi_printf("\n");
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$show";
    task.calltf = show_arguments;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
