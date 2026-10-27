/* ADV-015 checker instances appear as scopes in the VPI hierarchy. */
#include "vpi_user.h"

static void show(const char* path) {
    vpiHandle scope = vpi_handle_by_name((PLI_BYTE8*)path, NULL);
    if (!scope) {
        vpi_printf("%s missing\n", (PLI_BYTE8*)path);
        return;
    }
    vpiHandle parent = vpi_handle(vpiParent, scope);
    int variables = 0;
    vpiHandle iterator = vpi_iterate(vpiVariables, scope);
    if (iterator) {
        while (vpi_scan(iterator)) ++variables;
    }
    vpi_printf("%s def=%s parent=%s vars=%d\n", vpi_get_str(vpiFullName, scope),
               vpi_get_str(vpiDefName, scope),
               parent ? vpi_get_str(vpiFullName, parent) : "-", variables);
}

static void value(const char* path) {
    vpiHandle variable = vpi_handle_by_name((PLI_BYTE8*)path, NULL);
    s_vpi_value data = {0};
    data.format = vpiIntVal;
    if (variable) vpi_get_value(variable, &data);
    vpi_printf("%s=%d\n", (PLI_BYTE8*)path, variable ? data.value.integer : -1);
}

static PLI_INT32 checker_call(PLI_BYTE8* user_data) {
    show("tb.u_chk");
    show("tb.u_chk.u_inner");
    value("tb.u_chk.count");
    value("tb.u_chk.u_inner.hits");
    (void)user_data;
    return 0;
}

static void register_vpi(void) {
    s_vpi_systf_data task = {0};
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$vpi_checker";
    task.calltf = checker_call;
    (void)vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_vpi, NULL};
