/* ADV-019 family 3: walks the class definitions of classes.sv (IEEE
 * 1800-2009 37.27-37.29, 37.37, 37.26, 37.13): the definitions each scope
 * declares, their full names and scopes, the extends object, the methods
 * with their qualifiers and arguments, the parameters with their values,
 * and the randomization, visibility and const properties of class
 * properties and module variables. */
#include "sv_vpi_user.h"
#include <string.h>

static const char* str(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return vpi_chk_error(NULL) || !s ? "-" : s;
}

static int get(PLI_INT32 property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    return vpi_chk_error(NULL) ? -99 : value;
}

static const char* related(PLI_INT32 relation, vpiHandle h) {
    vpiHandle r = vpi_handle(relation, h);
    if (vpi_chk_error(NULL)) return "error";
    if (!r) return "null";
    const char* name = vpi_get_str(vpiFullName, r);
    if (vpi_chk_error(NULL) || !name) name = vpi_get_str(vpiName, r);
    return name ? name : "-";
}

static void list_classes(const char* label, vpiHandle scope) {
    vpiHandle it = vpi_iterate(vpiClassDefn, scope), c;
    vpi_printf("classes of %s:", label);
    while (it && (c = vpi_scan(it))) vpi_printf(" %s", str(vpiFullName, c));
    vpi_printf("\n");
}

static void arguments(vpiHandle method) {
    vpiHandle it = vpi_iterate(vpiIODecl, method), a;
    while (it && (a = vpi_scan(it))) {
        vpiHandle ts = vpi_handle(vpiTypespec, a);
        vpi_printf("    io %s dir=%d size=%d signed=%d typespec=%s taskfunc=%s expr=%s\n",
                   str(vpiName, a), get(vpiDirection, a), get(vpiSize, a), get(vpiSigned, a),
                   ts ? str(vpiType, ts) : "null", related(vpiTaskFunc, a), related(vpiExpr, a));
    }
}

static void methods(vpiHandle defn) {
    vpiHandle it = vpi_iterate(vpiMethods, defn), m;
    while (it && (m = vpi_scan(it))) {
        vpi_printf("  %s %s full=%s method=%d virtual=%d visibility=%d automatic=%d",
                   str(vpiType, m), str(vpiName, m), str(vpiFullName, m), get(vpiMethod, m),
                   get(vpiVirtual, m), get(vpiVisibility, m), get(vpiAutomatic, m));
        if (vpi_get(vpiType, m) == vpiFunction)
            vpi_printf(" functype=%d size=%d signed=%d", get(vpiFuncType, m), get(vpiSize, m),
                       get(vpiSigned, m));
        vpi_printf(" classdefn=%s scope=%s instance=%s line=%d\n", related(vpiClassDefn, m),
                   related(vpiScope, m), related(vpiInstance, m), get(vpiLineNo, m));
        vpi_handle(vpiStmt, m);
        vpi_printf("    stmt level=%d\n", vpi_chk_error(NULL));
        arguments(m);
    }
}

static void parameters(vpiHandle defn) {
    vpiHandle it = vpi_iterate(vpiParameter, defn), p;
    while (it && (p = vpi_scan(it))) {
        s_vpi_value v;
        v.format = vpiIntVal;
        vpi_get_value(p, &v);
        vpi_printf("  parameter %s full=%s local=%d consttype=%d size=%d value=%d scope=%s\n",
                   str(vpiName, p), str(vpiFullName, p), get(vpiLocalParam, p),
                   get(vpiConstType, p), get(vpiSize, p), (int)v.value.integer,
                   related(vpiScope, p));
    }
}

static void properties(vpiHandle owner) {
    vpiHandle it = vpi_iterate(vpiVariables, owner), p;
    while (it && (p = vpi_scan(it))) {
        vpi_printf("  property %s randtype=%d randomized=%d visibility=%d const=%d\n",
                   str(vpiName, p), get(vpiRandType, p), get(vpiIsRandomized, p),
                   get(vpiVisibility, p), get(vpiConstantVariable, p));
    }
}

static void definition(vpiHandle defn) {
    vpi_printf("defn %s full=%s virtual=%d automatic=%d scope=%s instance=%s module=%s\n",
               str(vpiName, defn), str(vpiFullName, defn), get(vpiVirtual, defn),
               get(vpiAutomatic, defn), related(vpiScope, defn), related(vpiInstance, defn),
               related(vpiModule, defn));
    vpiHandle extends = vpi_handle(vpiExtends, defn);
    if (extends) {
        vpiHandle base = vpi_handle(vpiClassTypespec, extends);
        vpiHandle base_defn = base ? vpi_handle(vpiClassDefn, base) : NULL;
        vpi_printf("  extends %s -> %s\n", str(vpiType, extends),
                   base_defn ? str(vpiFullName, base_defn) : "null");
    }
    vpiHandle it = vpi_iterate(vpiDerivedClasses, defn), d;
    while (it && (d = vpi_scan(it))) vpi_printf("  derived %s\n", str(vpiFullName, d));
    methods(defn);
    parameters(defn);
    properties(defn);
    it = vpi_iterate(vpiInternalScope, defn);
    int scopes = 0;
    while (it && vpi_scan(it)) ++scopes;
    vpi_printf("  internal scopes=%d\n", scopes);
    it = vpi_iterate(vpiNamedEvent, defn);
    vpi_printf("  events %s level=%d\n", it ? "some" : "none", vpi_chk_error(NULL));
    it = vpi_iterate(vpiPropertyDecl, defn);
    vpi_printf("  property decls %s level=%d\n", it ? "some" : "none", vpi_chk_error(NULL));
}

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    list_classes("NULL", NULL);
    vpiHandle tb = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    list_classes("tb", tb);
    vpiHandle it = vpi_iterate(vpiPackage, NULL), pkg;
    while (it && (pkg = vpi_scan(it))) list_classes(str(vpiName, pkg), pkg);
    vpi_iterate(vpiClassDefn, vpi_handle_by_name((PLI_BYTE8*)"tb.plain", NULL));
    vpi_printf("classes of a variable level=%d\n", vpi_chk_error(NULL));

    it = vpi_iterate(vpiClassDefn, NULL);
    vpiHandle c;
    while (it && (c = vpi_scan(it))) definition(c);

    vpiHandle var = vpi_handle_by_name((PLI_BYTE8*)"tb.obj", NULL);
    vpiHandle obj = vpi_handle(vpiClassObj, var);
    vpi_printf("object properties:\n");
    properties(obj);
    it = vpi_iterate(vpiMethods, obj);
    int count = 0;
    while (it && vpi_scan(it)) ++count;
    vpi_printf("object methods=%d\n", count);
    it = vpi_iterate(vpiParameter, obj);
    count = 0;
    while (it && vpi_scan(it)) ++count;
    vpi_printf("object parameters=%d\n", count);

    const char* names[] = {"tb.limit", "tb.plain", "tb.obj"};
    for (int i = 0; i < 3; ++i) {
        vpiHandle v = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        vpi_printf("variable %s randtype=%d randomized=%d visibility=%d const=%d\n", names[i],
                   get(vpiRandType, v), get(vpiIsRandomized, v), get(vpiVisibility, v),
                   get(vpiConstantVariable, v));
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$classes";
    task.calltf = walk;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
