/* Decision A19-D32: typespecs and expressions of expressions, variables
 * and calls (IEEE 1800-2009 37.17, 37.26, 37.38, 37.55). See
 * A19-D32_expression_relations.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle get(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static const char* text(int property, vpiHandle h) {
    const char* s = h ? vpi_get_str(property, h) : NULL;
    return level() || !s ? "-" : s;
}

/* The type of a one-to-one relation: its vpiType, 0 for NULL, or the
 * negated error level. */
static int related(int type, vpiHandle h) {
    vpiHandle r = vpi_handle(type, h);
    int error = level();
    if (error) return -error;
    return r ? (int)vpi_get(vpiType, r) : 0;
}

static void typespec(vpiHandle h) {
    vpi_printf(" typespec=%d", related(vpiTypespec, h));
}

static void related_text(const char* label, int type, vpiHandle h) {
    vpiHandle r = vpi_handle(type, h);
    int error = level();
    if (error)
        vpi_printf(" %s=error%d", label, error);
    else
        vpi_printf(" %s=%s", label, r ? text(vpiDecompile, r) : "NULL");
}

static int is_call(int type) {
    return type == vpiFuncCall || type == vpiSysFuncCall || type == vpiMethodFuncCall ||
           type == vpiTaskCall || type == vpiSysTaskCall || type == vpiMethodTaskCall;
}

static void call(vpiHandle h, int type) {
    int defn = (int)vpi_get(vpiUserDefn, h);
    int error = level();
    if (error)
        vpi_printf(" userdefn=error%d", error);
    else
        vpi_printf(" userdefn=%d", defn);
    if (type == vpiSysFuncCall || type == vpiSysTaskCall)
        vpi_printf(" systf=%d", related(vpiUserSystf, h));
    if (type == vpiFuncCall || type == vpiMethodFuncCall)
        vpi_printf(" function=%d", related(vpiFunction, h));
    if (type == vpiTaskCall || type == vpiMethodTaskCall)
        vpi_printf(" task=%d", related(vpiTask, h));
    if (type == vpiMethodFuncCall || type == vpiMethodTaskCall) {
        related_text("prefix", vpiPrefix, h);
        related_text("with", vpiWith, h);
    }
    vpi_printf(" args=");
    vpiHandle it = vpi_iterate(vpiArgument, h);
    level();
    vpiHandle a;
    while (it && (a = vpi_scan(it)) != NULL) vpi_printf("[%s]", text(vpiDecompile, a));
}

static void expression(const char* indent, vpiHandle h, int depth) {
    int type = (int)vpi_get(vpiType, h);
    vpi_printf("%stype=%d %s", indent, type, text(vpiDecompile, h));
    typespec(h);
    if (is_call(type)) call(h, type);
    vpi_printf("\n");
    if (type != vpiOperation || depth > 6) return;
    vpiHandle it = vpi_iterate(vpiOperand, h);
    vpiHandle o;
    while (it && (o = vpi_scan(it)) != NULL) expression(indent, o, depth + 1);
}

static void object(const char* name) {
    vpiHandle h = get(name);
    vpi_printf("%s type=%d %s", name, (int)vpi_get(vpiType, h), text(vpiDecompile, h));
    typespec(h);
    related_text("expr", vpiExpr, h);
    vpi_printf("\n");
}

static PLI_INT32 d32(PLI_BYTE8* data) {
    (void)data;
    const char* names[] = {"tb.r0", "tb.k", "tb.n", "tb.s", "tb.ch", "tb.p",
                           "tb.mem", "tb.W", "tb.D"};
    for (size_t i = 0; i < sizeof names / sizeof names[0]; ++i) object(names[i]);
    vpiHandle bit = vpi_handle_by_index(get("tb.r0"), 1);
    vpi_printf("tb.r0[1] type=%d", (int)vpi_get(vpiType, bit));
    typespec(bit);
    vpi_printf("\n");
    vpiHandle ports = vpi_iterate(vpiPort, get("tb.u"));
    vpiHandle port;
    while (ports && (port = vpi_scan(ports)) != NULL) {
        vpi_printf("port %s", text(vpiName, port));
        typespec(port);
        vpiHandle bits = vpi_iterate(vpiBit, port);
        vpiHandle first = bits ? vpi_scan(bits) : NULL;
        if (first) {
            vpi_printf(" bit");
            typespec(first);
            vpi_free_object(bits);
        }
        vpi_printf("\n");
    }
    vpiHandle stmts = vpi_iterate(vpiStmt, get("tb.main"));
    vpiHandle stmt;
    while (stmts && (stmt = vpi_scan(stmts)) != NULL) {
        int type = (int)vpi_get(vpiType, stmt);
        const char* lhs = type == vpiAssignment ? text(vpiName, vpi_handle(vpiLhs, stmt)) : "";
        if (strcmp(lhs, "n") == 0 || strcmp(lhs, "r") == 0) {
            vpi_printf("assign %s\n", text(vpiDecompile, vpi_handle(vpiLhs, stmt)));
            expression("  ", vpi_handle(vpiRhs, stmt), 0);
        } else if (is_call(type)) {
            expression("call ", stmt, 0);
        }
    }
    return 0;
}

static PLI_INT32 d32f(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = 5;
    vpi_put_value(vpi_handle(vpiSysTfCall, NULL), &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 d32f_size(PLI_BYTE8* data) {
    (void)data;
    return 32;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d32";
    task.calltf = d32;
    vpi_register_systf(&task);
    memset(&task, 0, sizeof task);
    task.type = vpiSysFunc;
    task.sysfunctype = vpiIntFunc;
    task.tfname = (PLI_BYTE8*)"$d32f";
    task.calltf = d32f;
    task.sizetf = d32f_size;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, 0};
