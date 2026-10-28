/* Companion of A19-D24_select_names.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

static PLI_INT32 d24(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL)), arg;
    while (it && (arg = vpi_scan(it))) {
        char name[128], full[128];
        snprintf(name, sizeof(name), "%s", text(vpi_get_str(vpiName, arg)));
        snprintf(full, sizeof(full), "%s", text(vpi_get_str(vpiFullName, arg)));
        vpiHandle found = vpi_handle_by_name(full, NULL);
        vpi_printf("name=%s full=%s by_name=%s\n", name, full, found ? "found" : "NULL");
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d24";
    task.calltf = d24;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
