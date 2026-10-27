/* Companion of A19-D5_cont_assign_value.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

static const char* binary(vpiHandle h) {
    s_vpi_value value;
    value.format = vpiBinStrVal;
    vpi_get_value(h, &value);
    return vpi_chk_error(NULL) ? "error" : value.value.str;
}

static PLI_INT32 d5(PLI_BYTE8* data) {
    (void)data;
    vpiHandle assigns = vpi_iterate(vpiContAssign, vpi_handle_by_name("tb", NULL)), assign;
    while ((assign = vpi_scan(assigns))) {
        vpiHandle lhs = vpi_handle(vpiLhs, assign), rhs = vpi_handle(vpiRhs, assign);
        vpi_printf("%s", text(vpi_get_str(vpiFullName, lhs)));
        vpi_printf(" = %s (%s)", text(vpi_get_str(vpiFullName, rhs)), binary(rhs));
        vpi_printf(" value=%s size=%d\n", binary(assign), (int)vpi_get(vpiSize, assign));
        const char* name = vpi_get_str(vpiName, assign);
        vpi_printf("  name=%s level=%d\n", text(name), (int)vpi_chk_error(NULL));
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d5";
    task.calltf = d5;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
