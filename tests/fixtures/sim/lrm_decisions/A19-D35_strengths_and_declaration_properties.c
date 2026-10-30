/* Decision A19-D35: drive strengths and declaration properties of nets and
 * ports (IEEE 1800-2009 37.14, 37.16, 37.31, 37.43). See
 * A19-D35_strengths_and_declaration_properties.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle get(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static void property(const char* name, vpiHandle h, const char* label, int property) {
    int value = (int)vpi_get(property, h);
    int error = level();
    vpi_printf("  %s %s %d level=%d\n", name, label, value, error);
}

static void net(const char* name, vpiHandle h) {
    vpi_printf("%s\n", name);
    property(name, h, "vpiStrength0", vpiStrength0);
    property(name, h, "vpiStrength1", vpiStrength1);
    property(name, h, "vpiNetDeclAssign", vpiNetDeclAssign);
    property(name, h, "vpiExplicitScalared", vpiExplicitScalared);
    property(name, h, "vpiExplicitVectored", vpiExplicitVectored);
    property(name, h, "vpiExpanded", vpiExpanded);
    property(name, h, "vpiImplicitDecl", vpiImplicitDecl);
    property(name, h, "vpiNetType", vpiNetType);
    property(name, h, "vpiResolvedNetType", vpiResolvedNetType);
}

static PLI_INT32 d35(PLI_BYTE8* data) {
    (void)data;
    static const char* const nets[] = {"tb.a",  "tb.b",   "tb.c",    "tb.f",    "tb.sc",
                                       "tb.vc", "tb.s0",  "tb.ta",   "tb.tt",   "tb.ext",
                                       "tb.p1.ti", "tb.imp", "tb.wi", "tb.i1.wi"};
    for (size_t i = 0; i < sizeof nets / sizeof nets[0]; ++i) net(nets[i], get(nets[i]));
    vpiHandle bits = vpi_iterate(vpiBit, get("tb.sc"));
    vpiHandle bit = bits ? vpi_scan(bits) : NULL;
    if (bits && bit) vpi_free_object(bits);
    if (bit) net("tb.sc bit", bit);
    vpiHandle prims = vpi_iterate(vpiPrimitive, get("tb"));
    for (vpiHandle prim; prims && (prim = vpi_scan(prims));) {
        char name[96];
        snprintf(name, sizeof name, "primitive type%d@%d", (int)vpi_get(vpiPrimType, prim),
                 (int)vpi_get(vpiLineNo, prim));
        vpi_printf("%s\n", name);
        property(name, prim, "vpiStrength0", vpiStrength0);
        property(name, prim, "vpiStrength1", vpiStrength1);
    }
    vpiHandle assigns = vpi_iterate(vpiContAssign, get("tb"));
    for (vpiHandle assign; assigns && (assign = vpi_scan(assigns));) {
        char name[64];
        snprintf(name, sizeof name, "assign@%d", (int)vpi_get(vpiLineNo, assign));
        vpi_printf("%s\n", name);
        property(name, assign, "vpiStrength0", vpiStrength0);
        property(name, assign, "vpiStrength1", vpiStrength1);
        property(name, assign, "vpiNetDeclAssign", vpiNetDeclAssign);
    }
    static const char* const instances[] = {"tb.p1", "tb.n1", "tb.n2", "tb.i1", "tb.i2"};
    for (size_t i = 0; i < sizeof instances / sizeof instances[0]; ++i) {
        vpiHandle ports = vpi_iterate(vpiPort, get(instances[i]));
        for (vpiHandle port; ports && (port = vpi_scan(ports));) {
            char name[96];
            snprintf(name, sizeof name, "%s port %s", instances[i], vpi_get_str(vpiName, port));
            vpi_printf("%s\n", name);
            property(name, port, "vpiExplicitName", vpiExplicitName);
            property(name, port, "vpiConnByName", vpiConnByName);
        }
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d35";
    task.calltf = d35;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
