/* ADV-019 typespec kinds (FIN-001 V3 family (d)): prints the typespec tree
 * of each object typespec_kinds.sv names and of the typedefs of tb
 * (IEEE 1800-2009 37.23): type, name, packed and tagged flags, typedef
 * aliases, enum bases, typespec members and array elements. */
#include "sv_vpi_user.h"
#include <string.h>

static const char* text(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return s ? s : "(null)";
}

static void typespec(const char* label, vpiHandle t, int depth) {
    if (!t) {
        /* The level of the vpi_handle call that produced t. */
        int level = vpi_chk_error(NULL);
        vpi_printf("%*s%s: NULL level=%d\n", 2 * depth, "", label, level);
        return;
    }
    int type = vpi_get(vpiType, t);
    vpi_printf("%*s%s: %s name=%s", 2 * depth, "", label, text(vpiType, t), text(vpiName, t));
    if (type == vpiLogicTypespec || type == vpiBitTypespec || type == vpiPackedArrayTypespec) {
        vpiHandle ranges = vpi_iterate(vpiRange, t), range;
        while ((range = vpi_scan(ranges))) {
            s_vpi_value left, right;
            left.format = right.format = vpiIntVal;
            vpi_get_value(vpi_handle(vpiLeftRange, range), &left);
            vpi_get_value(vpi_handle(vpiRightRange, range), &right);
            vpi_printf(" [%d:%d]", left.value.integer, right.value.integer);
        }
    }
    if (type == vpiStructTypespec || type == vpiUnionTypespec)
        vpi_printf(" packed=%d", vpi_get(vpiPacked, t));
    if (type == vpiUnionTypespec) vpi_printf(" tagged=%d", vpi_get(vpiTagged, t));
    vpi_printf("\n");
    vpiHandle alias = vpi_handle(vpiTypedefAlias, t);
    if (alias) typespec("alias", alias, depth + 1);
    if (type == vpiEnumTypespec) typespec("base", vpi_handle(vpiBaseTypespec, t), depth + 1);
    if (type == vpiArrayTypespec || type == vpiPackedArrayTypespec ||
        ((type == vpiLogicTypespec || type == vpiBitTypespec) && vpi_get(vpiVector, t)))
        typespec("elem", vpi_handle(vpiElemTypespec, t), depth + 1);
    if (type == vpiStructTypespec || type == vpiUnionTypespec) {
        vpiHandle members = vpi_iterate(vpiTypespecMember, t), member;
        while ((member = vpi_scan(members))) {
            vpi_printf("%*smember %s\n", 2 * depth + 2, "", text(vpiName, member));
            typespec("typespec", vpi_handle(vpiTypespec, member), depth + 2);
        }
    }
}

static void variable(const char* name) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpi_printf("%s %s\n", text(vpiFullName, h), text(vpiType, h));
    typespec("typespec", vpi_handle(vpiTypespec, h), 1);
}

/* The typedefs a scope declares (37.10 detail 1). */
static void typedefs(vpiHandle scope) {
    vpiHandle it = vpi_iterate(vpiTypedef, scope), t;
    while ((t = vpi_scan(it))) {
        vpi_printf("typedef line=%d\n", (int)vpi_get(vpiLineNo, t));
        typespec("typespec", t, 1);
    }
}

/* The properties of an object (37.29). */
static void properties(const char* name) {
    vpiHandle object = vpi_handle(vpiClassObj, vpi_handle_by_name((PLI_BYTE8*)name, NULL));
    vpiHandle it = vpi_iterate(vpiVariables, object), p;
    while ((p = vpi_scan(it))) {
        vpi_printf("property %s %s\n", text(vpiName, p), text(vpiType, p));
        typespec("typespec", vpi_handle(vpiTypespec, p), 1);
    }
}

/* The io declarations of a task (37.37, 37.13). */
static void formals(const char* name) {
    vpiHandle it = vpi_iterate(vpiIODecl, vpi_handle_by_name((PLI_BYTE8*)name, NULL)), d;
    while ((d = vpi_scan(it))) {
        vpi_printf("io %s\n", text(vpiName, d));
        typespec("typespec", vpi_handle(vpiTypespec, d), 1);
    }
}

/* The right-hand side of the first assignment of the initial block
 * (37.55 detail 5: a call has the typespec of its value). */
static void call_typespecs(vpiHandle top) {
    vpiHandle procs = vpi_iterate(vpiProcess, top), p;
    while ((p = vpi_scan(procs))) {
        vpiHandle stmts = vpi_iterate(vpiStmt, vpi_handle(vpiStmt, p)), s;
        while ((s = vpi_scan(stmts))) {
            if (vpi_get(vpiType, s) != vpiAssignment) continue;
            vpiHandle rhs = vpi_handle(vpiRhs, s);
            if (vpi_get(vpiType, rhs) != vpiFuncCall) continue;
            vpi_printf("call %s\n", text(vpiName, rhs));
            typespec("typespec", vpi_handle(vpiTypespec, rhs), 1);
        }
    }
}

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    static const char* names[] = {"tb.tp", "tb.tu", "tb.up", "tb.h", "tb.hn", "tb.hn2",
                                  "tb.sh", "tb.sh.h", "tb.th", "tb.vp", "tb.vu", "tb.hq",
                                  "tb.hq[0]", "tb.en", "tb.en2", "tb.em", "tb.ei",
                                  "tb.eq", NULL};
    vpiHandle top = vpi_handle_by_name("tb", NULL);
    typedefs(top);
    for (int i = 0; names[i]; ++i) variable(names[i]);
    properties("tb.c");
    formals("tb.take");
    call_typespecs(top);
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$typespec_kinds";
    task.calltf = walk;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
