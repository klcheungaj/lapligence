/* ADV-019 family 1: prints the typespec tree of each variable, parameter and
 * net of typespecs.sv (IEEE 1800-2009 37.23): type, name, packed and tagged
 * flags, ranges, typedef aliases, enum base and constants, typespec members
 * and the vpiElemTypespec chain. */
#include "sv_vpi_user.h"
#include <string.h>

static const char* text(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return s ? s : "(null)";
}

static int bound(vpiHandle range, PLI_INT32 side) {
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_handle(side, range), &value);
    return value.value.integer;
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
    if (type == vpiLogicTypespec || type == vpiBitTypespec || type == vpiPackedArrayTypespec)
        vpi_printf(" vector=%d", vpi_get(vpiVector, t));
    if (type == vpiArrayTypespec)
        vpi_printf(" static=%d", vpi_get(vpiArrayType, t) == vpiStaticArray);
    vpiHandle ranges = vpi_iterate(vpiRange, t), range;
    if (ranges) {
        vpi_printf(" ranges");
        while ((range = vpi_scan(ranges)))
            vpi_printf(" [%d:%d]", bound(range, vpiLeftRange), bound(range, vpiRightRange));
    }
    vpi_printf("\n");
    vpiHandle alias = vpi_handle(vpiTypedefAlias, t);
    if (alias) typespec("alias", alias, depth + 1);
    if (type == vpiEnumTypespec) {
        typespec("base", vpi_handle(vpiBaseTypespec, t), depth + 1);
        vpiHandle constants = vpi_iterate(vpiEnumConst, t), constant;
        while ((constant = vpi_scan(constants))) {
            s_vpi_value value;
            value.format = vpiBinStrVal;
            vpi_get_value(constant, &value);
            vpi_printf("%*sconst %s %s", 2 * depth + 2, "", text(vpiName, constant),
                       value.value.str);
            value.format = vpiIntVal;
            vpi_get_value(constant, &value);
            vpi_printf(" %d %s\n", value.value.integer, text(vpiType, constant));
        }
    }
    if (type == vpiStructTypespec || type == vpiUnionTypespec) {
        vpiHandle members = vpi_iterate(vpiTypespecMember, t), member;
        while ((member = vpi_scan(members))) {
            vpi_printf("%*smember %s %s\n", 2 * depth + 2, "", text(vpiType, member),
                       text(vpiName, member));
            typespec("typespec", vpi_handle(vpiTypespec, member), depth + 2);
        }
    }
    vpiHandle element = vpi_handle(vpiElemTypespec, t);
    if (element) typespec("elem", element, depth + 1);
}

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    static const char* names[] = {"c", "t", "n", "arr", "u", "cs", "b", "si", "li",
                                  "ig", "sc", "rl", "P", "w", NULL};
    vpiHandle top = vpi_handle_by_name("tb", NULL);
    for (int i = 0; names[i]; ++i) {
        vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)names[i], top);
        vpi_printf("%s %s\n", text(vpiFullName, h), text(vpiType, h));
        typespec("typespec", vpi_handle(vpiTypespec, h), 1);
    }
    /* Unsupported relations report an error instead of a fake handle. */
    vpiHandle enum_typespec = vpi_handle(vpiTypespec, vpi_handle_by_name("tb.c", NULL));
    vpiHandle none = vpi_handle(vpiIndexTypespec, enum_typespec);
    int level = vpi_chk_error(NULL);
    vpi_printf("vpiIndexTypespec of an enum typespec: %s level=%d\n", none ? "handle" : "NULL",
               level);
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$typespecs";
    task.calltf = walk;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
