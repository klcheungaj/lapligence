/* Companion of S42-D3_function_default_zero.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle saved;

static PLI_INT32 d3_compile(PLI_BYTE8* data) {
    (void)data;
    saved = vpi_handle(vpiSysTfCall, NULL);
    return 0;
}

static PLI_INT32 d3(PLI_BYTE8* data) {
    (void)data;
    return 0;
}

static PLI_INT32 d3_late(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiIntVal;
    v.value.integer = 5;
    vpiHandle result = vpi_put_value(saved, &v, NULL, vpiNoDelay);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("late put=%s level=%d\n", result ? "handle" : "null", (int)level);
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
    reg(vpiSysFunc, vpiIntFunc, "$d3", d3, d3_compile, NULL);
    reg(vpiSysTask, 0, "$d3_late", d3_late, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
