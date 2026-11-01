/* Decision A19-D66: the expression of a specparam.
 * Companion of A19-D66_specparam_expressions.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failed(void) {
    s_vpi_error_info error;
    return vpi_chk_error(&error) != 0;
}

static int int_value(vpiHandle handle) {
    s_vpi_value v;
    memset(&v, 0, sizeof v);
    v.format = vpiIntVal;
    vpi_get_value(handle, &v);
    return failed() ? -1 : (int)v.value.integer;
}

static PLI_INT32 d66(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiSpecParam, vpi_handle_by_name((PLI_BYTE8*)"tb", NULL)), param;
    while (it && (param = vpi_scan(it))) {
        vpi_printf("%s %s value=%d", (const char*)vpi_get_str(vpiType, param),
                   (const char*)vpi_get_str(vpiFullName, param), int_value(param));
        vpiHandle expr = vpi_handle(vpiExpr, param);
        int error = failed();
        if (!expr) {
            vpi_printf(" expr NULL%s\n", error ? "(error)" : "");
            continue;
        }
        vpi_printf(" expr %s value=%d", (const char*)vpi_get_str(vpiType, expr), int_value(expr));
        if (vpi_get(vpiType, expr) == vpiOperation) {
            vpi_printf(" op=%d operands", (int)vpi_get(vpiOpType, expr));
            vpiHandle operands = vpi_iterate(vpiOperand, expr), operand;
            while (operands && (operand = vpi_scan(operands)))
                vpi_printf(" %s=%d", (const char*)vpi_get_str(vpiType, operand), int_value(operand));
        }
        vpi_printf("\n");
    }
    return 0;
}

static void register_d66(void) {
    s_vpi_systf_data task = {vpiSysTask, 0, (PLI_BYTE8*)"$d66", d66, NULL, NULL, NULL};
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_d66, NULL};
