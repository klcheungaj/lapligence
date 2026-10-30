/* Decision A19-D36: class definitions, methods and the randomization,
 * visibility and const properties of variables (IEEE 1800-2009 37.10,
 * 37.17, 37.27, 37.37). See A19-D36_class_definitions.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static const char* str(PLI_INT32 property, vpiHandle h) {
    const char* s = h ? vpi_get_str(property, h) : NULL;
    return level() || !s ? "-" : s;
}

static int get(PLI_INT32 property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    return level() ? -99 : value;
}

/* The full name (or name, or type) of the object `relation` leads to. */
static const char* related(PLI_INT32 relation, vpiHandle h) {
    vpiHandle r = vpi_handle(relation, h);
    if (level()) return "error";
    if (!r) return "null";
    const char* name = vpi_get_str(vpiFullName, r);
    if (level() || !name) name = vpi_get_str(vpiType, r);
    return name ? name : "-";
}

static vpiHandle find_class(const char* full_name) {
    vpiHandle it = vpi_iterate(vpiClassDefn, NULL), c;
    while (it && (c = vpi_scan(it))) {
        const char* name = vpi_get_str(vpiFullName, c);
        if (name && !strcmp(name, full_name)) {
            vpi_release_handle(it);
            return c;
        }
    }
    return NULL;
}

static void qualifiers(const char* label, vpiHandle v) {
    vpi_printf("%s randtype=%d randomized=%d visibility=%d const=%d\n", label,
               get(vpiRandType, v), get(vpiIsRandomized, v), get(vpiVisibility, v),
               get(vpiConstantVariable, v));
}

static PLI_INT32 d36(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiClassDefn, NULL), c;
    int count = 0;
    while (it && (c = vpi_scan(it))) {
        vpi_printf("class %s name=%s scope=%s module=%s\n", str(vpiFullName, c), str(vpiName, c),
                   related(vpiScope, c), related(vpiModule, c));
        ++count;
    }
    vpi_printf("classes=%d\n", count);

    vpiHandle base = find_class("tb.Base");
    vpiHandle child = find_class("tb.Child");
    vpiHandle methods = vpi_iterate(vpiMethods, base), m;
    while (methods && (m = vpi_scan(methods))) {
        vpi_printf("method %s full=%s scope=%s parent=%s functype=%d\n", str(vpiName, m),
                   str(vpiFullName, m), related(vpiScope, m), related(vpiParent, m),
                   get(vpiFuncType, m));
        vpiHandle args = vpi_iterate(vpiIODecl, m), a;
        while (args && (a = vpi_scan(args)))
            vpi_printf("  io %s direction=%d taskfunc=%s\n", str(vpiName, a),
                       get(vpiDirection, a), related(vpiTaskFunc, a));
    }
    int scopes = 0;
    vpiHandle internal = vpi_iterate(vpiInternalScope, base), s;
    while (internal && (s = vpi_scan(internal))) {
        vpi_printf("internal scope %s\n", str(vpiFullName, s));
        ++scopes;
    }
    vpi_printf("internal scopes=%d\n", scopes);

    vpiHandle extends = vpi_handle(vpiExtends, child);
    vpi_printf("extends type=%s base=%s\n", extends ? str(vpiType, extends) : "null",
               extends ? related(vpiClassTypespec, extends) : "null");
    vpi_printf("base extends=%s\n", related(vpiExtends, base));

    vpiHandle props = vpi_iterate(vpiVariables, base), p;
    while (props && (p = vpi_scan(props))) {
        char label[64];
        snprintf(label, sizeof label, "defn %s", str(vpiName, p));
        qualifiers(label, p);
    }
    vpiHandle obj = vpi_handle_by_name((PLI_BYTE8*)"tb.obj", NULL);
    vpiHandle object = obj ? vpi_handle(vpiClassObj, obj) : NULL;
    props = object ? vpi_iterate(vpiVariables, object) : NULL;
    while (props && (p = vpi_scan(props))) {
        char label[64];
        snprintf(label, sizeof label, "object %s", str(vpiName, p));
        qualifiers(label, p);
    }
    qualifiers("variable tb.limit", vpi_handle_by_name((PLI_BYTE8*)"tb.limit", NULL));
    qualifiers("variable tb.plain", vpi_handle_by_name((PLI_BYTE8*)"tb.plain", NULL));
    qualifiers("variable tb.obj", obj);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task = {0};
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d36";
    task.calltf = d36;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
