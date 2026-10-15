/* Companion of S42-D10_userdata_per_call_site.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static int counters[4];
static int sites;
static vpiHandle first_handle;

static PLI_INT32 d10_compile(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    if (vpi_get_userdata(call) != NULL || sites >= 4) return 1;
    return vpi_put_userdata(call, &counters[sites++]) == 1 ? 0 : 1;
}

static PLI_INT32 d10(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    int* counter = (int*)vpi_get_userdata(call);
    if (!first_handle) first_handle = call;
    vpi_printf("site=%d calls=%d same as first=%d\n", (int)(counter - counters) + 1, ++*counter,
               (int)vpi_compare_objects(call, first_handle));
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
    reg(vpiSysTask, 0, "$d10", d10, d10_compile, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
