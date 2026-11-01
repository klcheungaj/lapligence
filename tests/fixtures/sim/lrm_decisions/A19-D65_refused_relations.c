/* Decision A19-D65: relations of the object diagrams refused with an
 * error. Companion of A19-D65_refused_relations.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failed(void) {
    s_vpi_error_info error;
    return vpi_chk_error(&error) != 0;
}

static vpiHandle get(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static void handle(const char* label, int relation, vpiHandle reference) {
    vpiHandle result = vpi_handle(relation, reference);
    int error = failed();
    vpi_printf("%s: %s%s\n", label, result ? "handle" : "NULL", error ? " error" : "");
}

static void iterate(const char* label, int relation, vpiHandle reference) {
    vpiHandle result = vpi_iterate(relation, reference);
    int error = failed();
    vpi_printf("%s: %s%s\n", label, result ? "iterator" : "NULL", error ? " error" : "");
    if (result) vpi_free_object(result);
}

static void value(const char* label, vpiHandle object) {
    s_vpi_value v;
    memset(&v, 0, sizeof v);
    v.format = vpiObjTypeVal;
    vpi_get_value(object, &v);
    int error = failed();
    vpi_printf("%s: %s\n", label, error ? "error" : "value");
}

static vpiHandle first(int relation, vpiHandle reference) {
    vpiHandle it = vpi_iterate(relation, reference);
    vpiHandle item = it ? vpi_scan(it) : NULL;
    if (item) vpi_free_object(it);
    return item;
}

/* The value of each function call assigned in `stmt`, blocks searched. */
static void calls(vpiHandle stmt) {
    int type = (int)vpi_get(vpiType, stmt);
    if (type == vpiBegin || type == vpiNamedBegin) {
        vpiHandle it = vpi_iterate(vpiStmt, stmt), child;
        while (it && (child = vpi_scan(it))) calls(child);
    } else if (type == vpiAssignment) {
        vpiHandle rhs = vpi_handle(vpiRhs, stmt);
        if (rhs && vpi_get(vpiType, rhs) == vpiFuncCall) value("function call value", rhs);
    }
}

static PLI_INT32 d65(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = get("tb");
    handle("active time format", vpiActiveTimeFormat, NULL);
    iterate("tb alias statements", vpiAliasStmt, tb);
    iterate("tb let declarations", vpiLetDecl, tb);
    vpi_printf("let by name: %s\n", get("tb.inc") ? "handle" : "NULL");
    vpiHandle modport = get("tb.i0.mp");
    PLI_INT32 name = vpi_get(vpiName, modport);
    int error = failed();
    vpi_printf("modport vpi_get(vpiName): %d%s\n", (int)name, error ? " error" : "");
    vpiHandle port = first(vpiPort, get("tb.u"));
    vpiHandle bit = vpi_handle_by_index(port, 0);
    error = failed();
    vpi_printf("port by index: %s%s\n", bit ? "handle" : "NULL", error ? " error" : "");
    static const char* const typed[] = {"tb.c", "tb.sv"};
    for (unsigned i = 0; i < sizeof typed / sizeof typed[0]; ++i) {
        vpiHandle typespec = vpi_handle(vpiTypespec, get(typed[i]));
        char label[64];
        snprintf(label, sizeof label, "%s typespec instance", typed[i]);
        handle(label, vpiInstance, typespec);
        snprintf(label, sizeof label, "%s typespec module", typed[i]);
        handle(label, vpiModule, typespec);
        snprintf(label, sizeof label, "%s typespec scope", typed[i]);
        handle(label, vpiScope, typespec);
    }
    vpiHandle constant = first(vpiEnumConst, vpi_handle(vpiTypespec, get("tb.c")));
    handle("enum constant typespec", vpiEnumTypespec, constant);
    value("tb.mem value", get("tb.mem"));
    value("tb.nets value", get("tb.nets"));
    vpiHandle d = get("tb.d");
    value("tb.d value", d);
    vpiHandle object = vpi_handle(vpiClassObj, d);
    vpiHandle spec = vpi_handle(vpiTypespec, d);
    vpiHandle defn = vpi_handle(vpiClassDefn, spec);
    iterate("class defn memories", vpiMemory, defn);
    iterate("class typespec memories", vpiMemory, spec);
    iterate("class object memories", vpiMemory, object);
    iterate("class typespec typedefs", vpiTypedef, spec);
    iterate("class object typedefs", vpiTypedef, object);
    iterate("class typespec parameter assignments", vpiParamAssign, spec);
    handle("class typespec typedef alias", vpiTypedefAlias, spec);
    handle("class object scope", vpiScope, object);
    const char* full = vpi_get_str(vpiFullName, object);
    error = failed();
    vpi_printf("class object full name: %s%s\n", full ? full : "NULL", error ? " error" : "");
    calls(vpi_handle(vpiStmt, first(vpiProcess, tb)));
    (void)failed();
    return 0;
}

static void register_d65(void) {
    s_vpi_systf_data task = {vpiSysTask, 0, (PLI_BYTE8*)"$d65", d65, NULL, NULL, NULL};
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_d65, NULL};
