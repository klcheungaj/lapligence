/* VPI application of vpi.sv. */
#include "sv_vpi_user.h"
#include <string.h>

static void show(vpiHandle parameter) {
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    vpi_printf("%s type=%d size=%d local=%d", vpi_get_str(vpiFullName, parameter),
               vpi_get(vpiType, parameter), vpi_get(vpiSize, parameter),
               vpi_get(vpiLocalParam, parameter));
    if (vpi_get(vpiConstType, parameter) == vpiRealConst) {
        value.format = vpiRealVal;
        vpi_get_value(parameter, &value);
        vpi_printf(" %.2f\n", value.value.real);
    } else {
        value.format = vpiHexStrVal;
        vpi_get_value(parameter, &value);
        vpi_printf(" %s\n", value.value.str);
    }
}

static PLI_INT32 start(p_cb_data data) {
    static const char* const scopes[] = {"tb", "tb.b", "tb.b.c", "tb.f", "tb.fk"};
    static const char* const names[] = {"tb.b.K", "tb.b.c.K", "tb.f.FK", "tb.fk.FF"};
    (void)data;
    for (size_t index = 0; index < sizeof(scopes) / sizeof(*scopes); ++index) {
        vpiHandle scope = vpi_handle_by_name((char*)scopes[index], NULL), parameter;
        vpiHandle iterator = vpi_iterate(vpiParameter, scope);
        vpi_printf("%s\n", scopes[index]);
        while (iterator && (parameter = vpi_scan(iterator))) show(parameter);
    }
    for (size_t index = 0; index < sizeof(names) / sizeof(*names); ++index) {
        vpiHandle parameter = vpi_handle_by_name((char*)names[index], NULL);
        vpi_printf("in %s: ", vpi_get_str(vpiFullName, vpi_handle(vpiScope, parameter)));
        show(parameter);
    }
    return 0;
}

static PLI_INT32 show_argument(PLI_BYTE8* user) {
    vpiHandle iterator = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    (void)user;
    show(vpi_scan(iterator));
    vpi_free_object(iterator);
    return 0;
}

static void registration(void) {
    s_cb_data callback;
    s_vpi_systf_data task;
    memset(&callback, 0, sizeof(callback));
    callback.reason = cbStartOfSimulation;
    callback.cb_rtn = start;
    vpi_register_cb(&callback);
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$show";
    task.calltf = show_argument;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
