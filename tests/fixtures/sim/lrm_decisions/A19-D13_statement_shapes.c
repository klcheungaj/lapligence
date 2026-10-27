/* Companion of A19-D13_statement_shapes.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

static void stmt(vpiHandle h, int depth) {
    PLI_INT32 type = vpi_get(vpiType, h);
    vpi_printf("%*s%s line=%d", depth * 2, "", vpi_get_str(vpiType, h), (int)vpi_get(vpiLineNo, h));
    if (type == vpiAssignment) {
        vpiHandle rhs = vpi_handle(vpiRhs, h);
        vpi_printf(" op=%d lhs=%s rhs=%s", (int)vpi_get(vpiOpType, h),
                   text(vpi_get_str(vpiDecompile, vpi_handle(vpiLhs, h))),
                   rhs ? text(vpi_get_str(vpiDecompile, rhs)) : "NULL");
    }
    if (type == vpiEventControl) {
        vpiHandle condition = vpi_handle(vpiCondition, h);
        vpi_printf(" event=%s op=%d operands=", text(vpi_get_str(vpiDecompile, condition)),
                   (int)vpi_get(vpiOpType, condition));
        vpiHandle it = vpi_iterate(vpiOperand, condition), operand;
        int count = 0;
        while (it && (operand = vpi_scan(it))) ++count;
        vpi_printf("%d", count);
    }
    if (type == vpiIf || type == vpiCase) vpi_printf(" qualifier=%d", (int)vpi_get(vpiQualifier, h));
    vpi_printf("\n");
    if (type == vpiBegin || type == vpiNamedBegin) {
        vpiHandle it = vpi_iterate(vpiStmt, h), child;
        while (it && (child = vpi_scan(it))) stmt(child, depth + 1);
    } else if (type == vpiDelayControl || type == vpiEventControl || type == vpiIf) {
        vpiHandle child = vpi_handle(vpiStmt, h);
        if (child) stmt(child, depth + 1);
    }
}

static PLI_INT32 d13(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = vpi_handle_by_name("tb", NULL);
    vpiHandle processes = vpi_iterate(vpiProcess, tb);
    vpiHandle first = vpi_scan(processes);
    vpi_free_object(processes);
    stmt(vpi_handle(vpiStmt, first), 0);
    vpiHandle two = vpi_handle_by_name("tb.two", NULL);
    vpi_printf("task two line=%d\n", (int)vpi_get(vpiLineNo, two));
    stmt(vpi_handle(vpiStmt, two), 1);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d13";
    task.calltf = d13;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
