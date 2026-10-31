/* Decision A19-D61: the order of the connectivity iterations of a net
 * (IEEE 1800-2009 37.16). Companion of A19-D61_connectivity_order.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>
static void show(vpiHandle h) {
    int type = (int)vpi_get(vpiType, h);
    if (type == vpiContAssign) {
        vpiHandle lhs = vpi_handle(vpiLhs, h);
        vpiHandle rhs = vpi_handle(vpiRhs, h);
        vpiHandle scope = vpi_handle(vpiModule, h);
        vpi_printf(" assign:%s=%s@%s", (const char*)vpi_get_str(vpiDecompile, lhs),
                   (const char*)vpi_get_str(vpiDecompile, rhs),
                   (const char*)vpi_get_str(vpiFullName, scope));
    } else if (type == vpiPrimTerm) {
        vpiHandle primitive = vpi_handle(vpiPrimitive, h);
        vpi_printf(" term%d:%s", (int)vpi_get(vpiTermIndex, h),
                   (const char*)vpi_get_str(vpiFullName, primitive));
    } else {
        vpi_printf(" %s:%s", (const char*)vpi_get_str(vpiType, h),
                   (const char*)vpi_get_str(vpiFullName, h));
    }
}
static void list(const char* label, int relation, vpiHandle h) {
    vpi_printf("  %s:", label);
    vpiHandle it = vpi_iterate(relation, h), item;
    while (it && (item = vpi_scan(it))) show(item);
    vpi_printf("\n");
}
static PLI_INT32 d61(PLI_BYTE8* data) {
    (void)data;
    const char* names[] = {"tb.n", "tb.k2.i", "tb.gg.cc.o"};
    for (unsigned i = 0; i < sizeof names / sizeof *names; ++i) {
        vpiHandle net = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        vpi_printf("%s\n", names[i]);
        list("vpiDriver", vpiDriver, net);
        list("vpiLoad", vpiLoad, net);
        list("vpiLocalDriver", vpiLocalDriver, net);
        list("vpiLocalLoad", vpiLocalLoad, net);
        list("vpiContAssign", vpiContAssign, net);
        list("vpiPrimTerm", vpiPrimTerm, net);
        list("vpiPortInst", vpiPortInst, net);
    }
    return 0;
}
static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d61";
    task.calltf = d61;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, 0};
