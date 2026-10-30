/* Decision A19-D37: constraints as VPI objects (IEEE 1800-2009 37.30,
 * 37.34, 37.3.7). See A19-D37_constraints.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static int get(PLI_INT32 property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    return level() ? -99 : value;
}

static const char* str(PLI_INT32 property, vpiHandle h) {
    const char* s = vpi_get_str(property, h);
    return level() || !s ? "-" : s;
}

static void show(const char* label, vpiHandle owner) {
    vpiHandle it = vpi_iterate(vpiConstraint, owner), c;
    while (it && (c = vpi_scan(it))) {
        vpi_printf("%s %s full=%s automatic=%d alloc=%d enabled=%d parent=%s\n", label,
                   str(vpiName, c), str(vpiFullName, c), get(vpiAutomatic, c),
                   get(vpiAllocScheme, c), get(vpiIsConstraintEnabled, c),
                   str(vpiType, vpi_handle(vpiParent, c)));
        vpiHandle items = vpi_iterate(vpiConstraintItem, c), item;
        while (items && (item = vpi_scan(items))) {
            vpi_printf("  item %s", str(vpiType, item));
            vpi_handle(vpiScope, item);
            vpi_printf(" scope level=%d", level());
            if (vpi_get(vpiType, item) == vpiDistribution) {
                vpiHandle dist = vpi_iterate(vpiDistItem, item), d;
                while (dist && (d = vpi_scan(dist))) {
                    vpiHandle range = vpi_handle(vpiValueRange, d);
                    vpiHandle weight = vpi_handle(vpiWeight, d);
                    vpi_printf(" [%s disttype=%d weight=%s]", str(vpiType, range),
                               get(vpiDistType, d), weight ? str(vpiDecompile, weight) : "null");
                }
            } else {
                vpi_printf(" %s", str(vpiDecompile, item));
            }
            vpi_printf("\n");
        }
    }
}

static PLI_INT32 d37(PLI_BYTE8* data) {
    (void)data;
    vpiHandle var = vpi_handle_by_name((PLI_BYTE8*)"tb.it", NULL);
    vpiHandle object = vpi_handle(vpiClassObj, var);
    vpiHandle defn = vpi_handle(vpiClassDefn, vpi_handle(vpiClassTypespec, object));
    show("defn", defn);
    show("object", object);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task = {0};
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d37";
    task.calltf = d37;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
