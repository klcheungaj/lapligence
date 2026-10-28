/* Companion of A19-D11_decompile_format.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static PLI_INT32 d11(PLI_BYTE8* data) {
    (void)data;
    vpiHandle assigns = vpi_iterate(vpiContAssign, vpi_handle_by_name("tb", NULL)), assign;
    while ((assign = vpi_scan(assigns))) {
        vpiHandle lhs = vpi_handle(vpiLhs, assign), rhs = vpi_handle(vpiRhs, assign);
        const char* decompiled = vpi_get_str(vpiDecompile, rhs);
        vpi_printf("%s = [%s]\n", vpi_get_str(vpiName, lhs), decompiled ? decompiled : "NULL");
    }
    vpiHandle a = vpi_handle_by_name("tb.a", NULL);
    vpi_printf("tb.a = [%s]\n", vpi_get_str(vpiDecompile, a));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d11";
    task.calltf = d11;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
