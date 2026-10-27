/* Companion of A19-D10_tchk_objects.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static void expression(vpiHandle h) {
    PLI_INT32 type = vpi_get(vpiType, h);
    if (type == vpiTchkTerm) {
        vpi_printf(" term");
    } else if (type == vpiConstant) {
        s_vpi_value value;
        value.format = vpiIntVal;
        vpi_get_value(h, &value);
        vpi_printf(" const(%d)", (int)value.value.integer);
    } else {
        const char* name = (const char*)vpi_get_str(vpiFullName, h);
        vpi_printf(" %s", name ? name : "expr");
    }
}

static void term(const char* label, vpiHandle h) {
    if (!h) {
        vpi_printf(" %s=NULL", label);
        return;
    }
    vpiHandle condition = vpi_handle(vpiCondition, h);
    const char* cond = condition ? (const char*)vpi_get_str(vpiFullName, condition) : "NULL";
    vpi_printf(" %s=%s edge=0x%02x cond=%s", label,
               (const char*)vpi_get_str(vpiFullName, vpi_handle(vpiExpr, h)),
               (int)vpi_get(vpiEdge, h), cond ? cond : "expr");
}

static PLI_INT32 d10(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiTchk, vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL)), tchk;
    while (it && (tchk = vpi_scan(it))) {
        vpi_printf("tchk type=%d", (int)vpi_get(vpiTchkType, tchk));
        term("ref", vpi_handle(vpiTchkRefTerm, tchk));
        term("data", vpi_handle(vpiTchkDataTerm, tchk));
        vpiHandle notifier = vpi_handle(vpiTchkNotifier, tchk);
        vpi_printf(" notifier=%s\n  args:",
                   notifier ? (const char*)vpi_get_str(vpiFullName, notifier) : "NULL");
        vpiHandle args = vpi_iterate(vpiExpr, tchk), arg;
        while (args && (arg = vpi_scan(args))) expression(arg);
        vpi_printf("\n");
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d10";
    task.calltf = d10;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
