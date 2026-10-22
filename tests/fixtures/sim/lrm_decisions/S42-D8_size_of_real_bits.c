/* Companion of S42-D8_size_of_real_bits.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static void size(const char* name) {
    PLI_INT32 value = vpi_get(vpiSize, vpi_handle_by_name((PLI_BYTE8*)name, NULL));
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("%s size=%d level=%d\n", name, (int)value, (int)level);
}

static PLI_INT32 d8(PLI_BYTE8* data) {
    (void)data;
    size("tb.mem");
    size("tb.i");
    size("tb.r");
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
    reg(vpiSysTask, 0, "$d8", d8, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
