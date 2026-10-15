/* Companion of S42-D5_delayed_put_nba.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 d5(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiIntVal;
    v.value.integer = 5;
    s_vpi_time t;
    memset(&t, 0, sizeof(t));
    t.type = vpiSimTime;
    t.low = 2;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.d", NULL), &v, &t, vpiInertialDelay);
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
    reg(vpiSysTask, 0, "$d5", d5, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
