/* SIM-099 vpi_string_scope_args.sv companion (gate item G4): one calltf for
 * $sdf_annotate, $input, $log and the user task $show that prints each
 * argument: a scope as its module's full name, anything else as its
 * vpiStringVal. Portable C11. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 show_calltf(PLI_BYTE8* user_data)
{
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg;
    int position = 0;
    (void)user_data;
    vpi_printf("%s:", vpi_get_str(vpiName, call));
    while (args && (arg = vpi_scan(args)) != NULL) {
        if (vpi_get(vpiType, arg) == vpiModule) {
            vpi_printf(" [%d] module %s", position, vpi_get_str(vpiFullName, arg));
        } else {
            s_vpi_value value;
            value.format = vpiStringVal;
            vpi_get_value(arg, &value);
            vpi_printf(" [%d] type %d \"%s\"", position, (int)vpi_get(vpiType, arg),
                       value.value.str);
        }
        position++;
    }
    vpi_printf("\n");
    return 0;
}

static void register_task(const char* name)
{
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)name;
    data.calltf = show_calltf;
    vpi_register_systf(&data);
}

static void startup(void)
{
    register_task("$sdf_annotate");
    register_task("$input");
    register_task("$log");
    register_task("$show");
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
