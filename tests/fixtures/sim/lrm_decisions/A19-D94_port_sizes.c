#include "sv_vpi_user.h"
#include <string.h>

static int failed(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) != 0;
}

static void property(const char* label, PLI_INT32 prop, vpiHandle h) {
    PLI_INT32 value = vpi_get(prop, h);
    vpi_printf(" %s=%d%s", label, (int)value, failed() ? " (error)" : "");
}

static PLI_INT32 d94(PLI_BYTE8* data) {
    (void)data;
    vpiHandle module = vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL);
    vpiHandle ports = vpi_iterate(vpiPort, module), port;
    while (ports && (port = vpi_scan(ports)) != NULL) {
        const char* name = vpi_get_str(vpiName, port);
        vpi_printf("%s:", name ? name : "NULL");
        property("size", vpiSize, port);
        property("scalar", vpiScalar, port);
        property("vector", vpiVector, port);
        vpi_printf("\n");
    }
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d94";
    task.calltf = d94;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, NULL};
