/* Decision A19-D90: prints the typespec of chandle variables, of enums
 * with a typedef or a built-in base, of a packed tagged union with a void
 * member, and of a virtual interface property. */
#include "sv_vpi_user.h"
#include <string.h>

static const char* text(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return s ? s : "(null)";
}

static void typespec(const char* label, vpiHandle t, int depth) {
    if (!t) {
        int level = vpi_chk_error(NULL);
        vpi_printf("%*s%s: NULL level=%d\n", 2 * depth, "", label, level);
        return;
    }
    int type = vpi_get(vpiType, t);
    vpi_printf("%*s%s: %s name=%s", 2 * depth, "", label, text(vpiType, t), text(vpiName, t));
    if (type == vpiUnionTypespec)
        vpi_printf(" packed=%d tagged=%d", (int)vpi_get(vpiPacked, t), (int)vpi_get(vpiTagged, t));
    vpi_printf("\n");
    vpiHandle alias = vpi_handle(vpiTypedefAlias, t);
    if (alias) typespec("alias", alias, depth + 1);
    if (type == vpiEnumTypespec) typespec("base", vpi_handle(vpiBaseTypespec, t), depth + 1);
    if (type == vpiUnionTypespec) {
        vpiHandle members = vpi_iterate(vpiTypespecMember, t), member;
        while (members && (member = vpi_scan(members))) {
            vpi_printf("%*smember %s\n", 2 * depth + 2, "", text(vpiName, member));
            typespec("typespec", vpi_handle(vpiTypespec, member), depth + 2);
        }
    }
}

static PLI_INT32 d90(PLI_BYTE8* data) {
    (void)data;
    static const char* names[] = {"tb.h", "tb.hn", "tb.en", "tb.ei", "tb.vp", NULL};
    for (int i = 0; names[i]; ++i) {
        vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        vpi_printf("%s\n", text(vpiFullName, h));
        typespec("typespec", vpi_handle(vpiTypespec, h), 1);
    }
    vpiHandle object = vpi_handle(vpiClassObj, vpi_handle_by_name("tb.c", NULL));
    vpiHandle vifs = vpi_iterate(vpiInterfaceDecl, object), vif;
    while (vifs && (vif = vpi_scan(vifs))) {
        vpi_printf("property %s virtual=%d\n", text(vpiName, vif), (int)vpi_get(vpiVirtual, vif));
        vpiHandle t = vpi_handle(vpiTypespec, vif);
        int level = vpi_chk_error(NULL);
        vpi_printf("  typespec: %s level=%d\n", t ? "handle" : "NULL", level);
    }
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d90";
    task.calltf = d90;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
