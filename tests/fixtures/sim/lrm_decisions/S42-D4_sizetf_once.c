/* Companion of S42-D4_sizetf_once.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static int compiles, sizes, sized_with_call;

static PLI_INT32 d4_compile(PLI_BYTE8* data) {
    (void)data;
    ++compiles;
    return 0;
}

static PLI_INT32 d4_size(PLI_BYTE8* data) {
    (void)data;
    ++sizes;
    if (vpi_handle(vpiSysTfCall, NULL) != NULL) sized_with_call = 1;
    return 6;
}

static PLI_INT32 d4(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiIntVal;
    v.value.integer = -1;
    vpi_put_value(vpi_handle(vpiSysTfCall, NULL), &v, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 d4_report(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("compiletf=%d sizetf=%d sizetf saw a call=%d\n", compiles, sizes, sized_with_call);
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
    reg(vpiSysFunc, vpiSizedFunc, "$d4", d4, d4_compile, d4_size);
    reg(vpiSysTask, 0, "$d4_report", d4_report, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
