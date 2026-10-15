/* Companion of S42-D2_chk_error_level.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static PLI_INT32 d2(PLI_BYTE8* data) {
    (void)data;
    vpiHandle missing = vpi_handle_by_name((PLI_BYTE8*)"tb.none", NULL);
    PLI_INT32 level = vpi_chk_error(NULL);
    PLI_INT32 again = vpi_chk_error(NULL);
    PLI_INT32 size = vpi_get(vpiSize, vpi_handle_by_name((PLI_BYTE8*)"tb.v", NULL));
    PLI_INT32 after = vpi_chk_error(NULL);
    vpi_printf("missing=%s level=%d again=%d size=%d after=%d\n", missing ? "handle" : "null",
               (int)level, (int)again, (int)size, (int)after);
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
    reg(vpiSysTask, 0, "$d2", d2, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
