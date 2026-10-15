/* Companion of S42-D11_name_lookup_scope.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static void find(const char* name, vpiHandle scope) {
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)name, scope);
    if (!handle) {
        vpi_printf("%s -> not found level=%d\n", name, (int)vpi_chk_error(NULL));
        return;
    }
    char full[64];
    snprintf(full, sizeof(full), "%s", (const char*)vpi_get_str(vpiFullName, handle));
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiHexStrVal;
    vpi_get_value(handle, &v);
    vpi_printf("%s -> %s = %s\n", name, full, v.value.str ? (const char*)v.value.str : "-");
}

static PLI_INT32 d11(PLI_BYTE8* data) {
    (void)data;
    vpiHandle u = vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL);
    find("tb.v", NULL);
    find("v", NULL);
    find("c", u);
    find("v", u);
    find("tb.mem[1]", NULL);
    find("tb.v[7:4]", NULL);
    find("tb.v[0]", NULL);
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
    reg(vpiSysTask, 0, "$d11", d11, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
