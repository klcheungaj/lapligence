/* Companion of S42-D9_real_value_formats.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 d9(PLI_BYTE8* data) {
    (void)data;
    vpiHandle r = vpi_handle_by_name((PLI_BYTE8*)"tb.r", NULL);
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiRealVal;
    vpi_get_value(r, &v);
    vpi_printf("real=%.2f\n", v.value.real);
    v.format = vpiIntVal;
    vpi_get_value(r, &v);
    vpi_printf("int=%d\n", (int)v.value.integer);
    v.format = vpiObjTypeVal;
    vpi_get_value(r, &v);
    vpi_printf("objtype format=%d real=%.2f\n", (int)v.format, v.value.real);
    static const PLI_INT32 formats[] = {vpiBinStrVal, vpiDecStrVal, vpiStringVal, vpiVectorVal};
    for (int k = 0; k < 4; ++k) {
        memset(&v, 0, sizeof(v));
        v.format = formats[k];
        vpi_get_value(r, &v);
        vpi_printf("format %d level=%d\n", (int)formats[k], (int)vpi_chk_error(NULL));
    }
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
    reg(vpiSysTask, 0, "$d9", d9, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
