/* Companion of S42-D12_iteration_order.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static void names(const char* label, PLI_INT32 type, vpiHandle ref) {
    vpi_printf("%s:", label);
    vpiHandle it = vpi_iterate(type, ref);
    vpiHandle item;
    while (it && (item = vpi_scan(it)) != NULL) {
        s_vpi_value v;
        memset(&v, 0, sizeof(v));
        v.format = vpiDecStrVal;
        char name[32];
        snprintf(name, sizeof(name), "%s", (const char*)vpi_get_str(vpiName, item));
        if (type == vpiPort) {
            vpi_printf(" %s", name);
        } else {
            vpi_get_value(item, &v);
            vpi_printf(" %s=%s", name, v.value.str ? (const char*)v.value.str : "-");
        }
    }
    vpi_printf("\n");
}

static PLI_INT32 d12(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = vpi_handle_by_name((PLI_BYTE8*)"tb", NULL);
    vpi_printf("regs:");
    vpiHandle it = vpi_iterate(vpiReg, tb);
    vpiHandle item;
    while ((item = vpi_scan(it)) != NULL) vpi_printf(" %s", (const char*)vpi_get_str(vpiName, item));
    vpi_printf("\n");
    names("ports", vpiPort, vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL));
    names("bits of up", vpiBit, vpi_handle_by_name((PLI_BYTE8*)"tb.up", NULL));
    names("elements of grid", vpiReg, vpi_handle_by_name((PLI_BYTE8*)"tb.grid", NULL));
    return 0;
}

static void reg(PLI_INT32 type, PLI_INT32 functype, const char* name, PLI_INT32 (*calltf)(PLI_BYTE8*),
                PLI_INT32 (*compiletf)(PLI_BYTE8*), PLI_INT32 (*sizetf)(PLI_BYTE8*)) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = type;
    data.sysfunctype = functype;
    data.tfname = (PLI_BYTE8*)name;
    data.calltf = calltf;
    data.compiletf = compiletf;
    data.sizetf = sizetf;
    if (!vpi_register_systf(&data)) vpi_printf("registration of %s failed\n", name);
}

static void register_all(void) {
    reg(vpiSysTask, 0, "$d12", d12, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
