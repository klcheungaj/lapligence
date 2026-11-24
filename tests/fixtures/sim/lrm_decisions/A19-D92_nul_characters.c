/* Companion of A19-D92_nul_characters.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static void text(vpiHandle object, PLI_INT32 format) {
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = format;
    vpi_get_value(object, &value);
    if (format == vpiHexStrVal) {
        vpi_printf(" %s", value.value.str);
        return;
    }
    vpi_printf(" [");
    for (const char* at = value.value.str; *at; ++at) {
        unsigned char ch = (unsigned char)*at;
        if (ch < 32 || ch > 126) vpi_printf("<%02x>", ch);
        else vpi_printf("%c", ch);
    }
    vpi_printf("]");
}

static void show(vpiHandle object) {
    text(object, vpiStringVal);
    text(object, vpiHexStrVal);
    vpi_printf("\n");
}

static PLI_INT32 d92(PLI_BYTE8* data) {
    (void)data;
    vpiHandle arguments = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL)), argument;
    argument = vpi_scan(arguments);
    s_vpi_value tag;
    memset(&tag, 0, sizeof(tag));
    tag.format = vpiStringVal;
    vpi_get_value(argument, &tag);
    vpi_printf("%s\n", tag.value.str);
    while ((argument = vpi_scan(arguments))) show(argument);
    return 0;
}

static void statement(vpiHandle object) {
    if (vpi_get(vpiType, object) == vpiBegin) {
        vpiHandle statements = vpi_iterate(vpiStmt, object), next;
        while (statements && (next = vpi_scan(statements))) statement(next);
        return;
    }
    if (vpi_get(vpiType, object) != vpiAssignment) return;
    vpiHandle rhs = vpi_handle(vpiRhs, object);
    vpi_printf("line %d", vpi_get(vpiLineNo, object));
    if (vpi_get(vpiType, rhs) != vpiOperation) {
        show(rhs);
        return;
    }
    vpi_printf("\n");
    vpiHandle operands = vpi_iterate(vpiOperand, rhs), operand;
    while (operands && (operand = vpi_scan(operands))) show(operand);
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    vpiHandle processes = vpi_iterate(vpiProcess, vpi_handle_by_name("tb", NULL));
    statement(vpi_handle(vpiStmt, vpi_scan(processes)));
    vpi_free_object(processes);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    s_cb_data callback;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d92";
    task.calltf = d92;
    vpi_register_systf(&task);
    memset(&callback, 0, sizeof(callback));
    callback.reason = cbStartOfSimulation;
    callback.cb_rtn = start;
    vpi_register_cb(&callback);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
