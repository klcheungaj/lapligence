/* Companion of S42-D1_put_value_returns_null.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle put(PLI_INT32 value, unsigned delay, PLI_INT32 flags) {
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiIntVal;
    v.value.integer = value;
    s_vpi_time t;
    memset(&t, 0, sizeof(t));
    t.type = vpiSimTime;
    t.low = delay;
    return vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.d", NULL), &v, &t, flags);
}

static PLI_INT32 d1(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("nodelay=%s\n", put(1, 0, vpiNoDelay) ? "handle" : "null");
    vpi_printf("nodelay with return event=%s\n",
               put(1, 0, vpiNoDelay | vpiReturnEvent) ? "handle" : "null");
    vpi_printf("delayed=%s\n", put(2, 1, vpiTransportDelay) ? "handle" : "null");
    vpiHandle event = put(3, 2, vpiPureTransportDelay | vpiReturnEvent);
    vpi_printf("event=%s\n", event ? (const char*)vpi_get_str(vpiType, event) : "null");
    vpi_printf("release=%d\n", (int)vpi_release_handle(event));
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
    reg(vpiSysTask, 0, "$d1", d1, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
