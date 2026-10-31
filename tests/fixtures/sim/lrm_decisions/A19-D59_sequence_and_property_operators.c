/* Decision A19-D59: sequence and property operators as operations with
 * operands (IEEE 1800-2009 37.48, 37.50, 37.52). Companion of
 * A19-D59_sequence_and_property_operators.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>
static s_vpi_error_info e;
static void walk(const char* role, vpiHandle h, int depth);
static void all(int relation, const char* role, vpiHandle h, int depth) {
    vpiHandle it = vpi_iterate(relation, h), k;
    vpi_chk_error(&e);
    while (it && (k = vpi_scan(it))) walk(role, k, depth);
}
static void one(int relation, const char* role, vpiHandle h, int depth) {
    vpiHandle k = vpi_handle(relation, h);
    vpi_chk_error(&e);
    if (k) walk(role, k, depth);
}
static void walk(const char* role, vpiHandle h, int depth) {
    int type = (int)vpi_get(vpiType, h);
    char detail[64] = "";
    if (type == vpiOperation) {
        int op = (int)vpi_get(vpiOpType, h);
        int strong = (int)vpi_get(vpiOpStrong, h);
        int err = vpi_chk_error(&e);
        if (err) snprintf(detail, sizeof detail, " op=%d", op);
        else snprintf(detail, sizeof detail, " op=%d strong=%d", op, strong);
    } else if (type == vpiConstant) {
        snprintf(detail, sizeof detail, " const=%d", (int)vpi_get(vpiConstType, h));
    } else if (type == vpiSequenceInst) {
        snprintf(detail, sizeof detail, " %s", (const char*)vpi_get_str(vpiName, h));
    }
    const char* text = (const char*)vpi_get_str(vpiDecompile, h);
    int err = vpi_chk_error(&e);
    vpi_printf("%*s%s %s%s line=%d%s%s\n", 2 * depth, "", role, (const char*)vpi_get_str(vpiType, h),
               detail, (int)vpi_get(vpiLineNo, h), err ? "" : " : ", err ? "" : text);
    switch (type) {
        case vpiOperation:
            all(vpiOperand, "operand", h, depth + 1);
            all(vpiMatchItem, "match", h, depth + 1);
            break;
        case vpiSequenceInst:
            all(vpiMatchItem, "match", h, depth + 1);
            break;
        case vpiClockedProp:
        case vpiClockedSeq:
            one(vpiClockingEvent, "clock", h, depth + 1);
            one(type == vpiClockedSeq ? vpiExpr : vpiPropertyExpr, "body", h, depth + 1);
            break;
        case vpiCaseProperty:
            one(vpiCondition, "condition", h, depth + 1);
            all(vpiCasePropertyItem, "item", h, depth + 1);
            break;
        case vpiCasePropertyItem:
            all(vpiExpr, "expr", h, depth + 1);
            one(vpiPropertyExpr, "property", h, depth + 1);
            break;
        default:
            break;
    }
}
static PLI_INT32 d59(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiAssertion, vpi_handle_by_name((PLI_BYTE8*)"tb", NULL)), a;
    while (it && (a = vpi_scan(it))) {
        vpiHandle spec = vpi_handle(vpiProperty, a);
        vpiHandle property = spec ? vpi_handle(vpiPropertyExpr, spec) : NULL;
        vpi_printf("%s\n", (const char*)vpi_get_str(vpiName, a));
        if (property) walk("property", property, 1);
    }
    /* A sequence or property operation has no value. */
    vpiHandle spec = vpi_handle(vpiProperty, vpi_handle_by_name((PLI_BYTE8*)"tb.p01", NULL));
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiBinStrVal;
    vpiHandle property = vpi_handle(vpiPropertyExpr, spec);
    vpi_get_value(property, &value);
    int value_error = vpi_chk_error(&e) != 0;
    int size = (int)vpi_get(vpiSize, property);
    int size_error = vpi_chk_error(&e) != 0;
    vpi_printf("value error=%d size=%d size error=%d\n", value_error, size, size_error);
    return 0;
}
static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d59";
    task.calltf = d59;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, 0};
