/* Companion of S42-D5_delayed_put_active.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static void put(const char* name, PLI_INT32 integer, unsigned delay, PLI_INT32 flags) {
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiIntVal;
    v.value.integer = integer;
    s_vpi_time t;
    memset(&t, 0, sizeof(t));
    t.type = vpiSimTime;
    t.low = delay;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)name, NULL), &v, &t, flags);
}

static PLI_INT32 d5(PLI_BYTE8* data) {
    (void)data;
    put("tb.d", 5, 2, vpiInertialDelay);
    put("tb.e", 6, 2, vpiTransportDelay);
    put("tb.f", 7, 2, vpiPureTransportDelay);
    return 0;
}

static PLI_INT32 d5_zero(PLI_BYTE8* data) {
    (void)data;
    put("tb.z", 3, 0, vpiTransportDelay);
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
    reg(vpiSysTask, 0, "$d5_zero", d5_zero, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
