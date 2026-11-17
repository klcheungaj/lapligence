/* ADV-019 typespec kinds (FIN-001 V3 family (d)): prints the typespec tree
 * of each variable of typespec_kinds.sv (IEEE 1800-2009 37.23): type, name,
 * packed and tagged flags, typedef aliases, enum bases and typespec
 * members. */
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
    if (type == vpiStructTypespec || type == vpiUnionTypespec)
        vpi_printf(" packed=%d", vpi_get(vpiPacked, t));
    if (type == vpiUnionTypespec) vpi_printf(" tagged=%d", vpi_get(vpiTagged, t));
    vpi_printf("\n");
    vpiHandle alias = vpi_handle(vpiTypedefAlias, t);
    if (alias) typespec("alias", alias, depth + 1);
    if (type == vpiEnumTypespec) typespec("base", vpi_handle(vpiBaseTypespec, t), depth + 1);
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

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    static const char* names[] = {"tb.tp", "tb.tu", "tb.up", NULL};
    for (int i = 0; names[i]; ++i) variable(names[i]);
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
