/* Companion of S42-D6_decimal_string_xz.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static void show(const char* name) {
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiDecStrVal;
    vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)name, NULL), &v);
    vpi_printf(" %s=%s", name + 3, v.value.str ? (const char*)v.value.str : "<null>");
}

static PLI_INT32 d6(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("vpiDecStrVal:");
    show("tb.a");
    show("tb.b");
    show("tb.c");
    show("tb.e");
    show("tb.m");
    show("tb.s");
    vpi_printf("\n");
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
    reg(vpiSysTask, 0, "$d6", d6, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
