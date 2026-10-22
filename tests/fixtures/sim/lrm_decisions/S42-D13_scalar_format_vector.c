/* Companion of S42-D13_scalar_format_vector.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 d13(PLI_BYTE8* data) {
    (void)data;
    vpiHandle v = vpi_handle_by_name((PLI_BYTE8*)"tb.v", NULL);
    vpiHandle s = vpi_handle_by_name((PLI_BYTE8*)"tb.s", NULL);
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiScalarVal;
    vpi_get_value(v, &value);
    vpi_printf("get vector level=%d scalar=%d\n", (int)vpi_chk_error(NULL), (int)value.value.scalar);
    value.format = vpiScalarVal;
    value.value.scalar = vpiX;
    vpi_put_value(v, &value, NULL, vpiNoDelay);
    vpi_printf("put vector level=%d\n", (int)vpi_chk_error(NULL));
    vpi_put_value(s, &value, NULL, vpiNoDelay);
    vpi_printf("put scalar level=%d\n", (int)vpi_chk_error(NULL));
    value.value.scalar = vpiH;
    vpi_put_value(vpi_handle_by_index(v, 1), &value, NULL, vpiNoDelay);
    vpi_printf("put bit level=%d\n", (int)vpi_chk_error(NULL));
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
    reg(vpiSysTask, 0, "$d13", d13, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
