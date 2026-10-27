/* Companion of A19-D12_expression_values.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static void show(const char* label, vpiHandle h) {
    s_vpi_value value;
    value.format = vpiObjTypeVal;
    vpi_get_value(h, &value);
    int level = vpi_chk_error(NULL);
    vpi_printf("%s %s size=%d", label, vpi_get_str(vpiType, h), (int)vpi_get(vpiSize, h));
    if (level) {
        vpi_printf(" value=error level=%d\n", level);
        return;
    }
    if (value.format == vpiRealVal) {
        vpi_printf(" value=%g\n", value.value.real);
        return;
    }
    value.format = vpiBinStrVal;
    vpi_get_value(h, &value);
    vpi_printf(" value=%s\n", value.value.str);
}

static PLI_INT32 d12(PLI_BYTE8* data) {
    (void)data;
    vpiHandle assigns = vpi_iterate(vpiContAssign, vpi_handle_by_name("tb", NULL)), assign;
    while ((assign = vpi_scan(assigns))) {
        vpiHandle rhs = vpi_handle(vpiRhs, assign);
        vpi_printf("%s:\n", vpi_get_str(vpiDecompile, rhs));
        show("  rhs", rhs);
        vpiHandle operands = vpi_iterate(vpiOperand, rhs), operand;
        if (!operands) vpi_chk_error(NULL);
        while (operands && (operand = vpi_scan(operands))) show("    operand", operand);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d12";
    task.calltf = d12;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
