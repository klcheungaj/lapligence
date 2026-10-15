/* Companion of S42-D7_index_out_of_range.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static void check(const char* label, vpiHandle handle) {
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("%s=%s level=%d\n", label, handle ? "handle" : "null", (int)level);
}

static PLI_INT32 d7(PLI_BYTE8* data) {
    (void)data;
    vpiHandle mem = vpi_handle_by_name((PLI_BYTE8*)"tb.mem", NULL);
    vpiHandle grid = vpi_handle_by_name((PLI_BYTE8*)"tb.grid", NULL);
    vpiHandle v = vpi_handle_by_name((PLI_BYTE8*)"tb.v", NULL);
    check("mem[1]", vpi_handle_by_index(mem, 1));
    check("mem[0]", vpi_handle_by_index(mem, 0));
    check("mem[5]", vpi_handle_by_index(mem, 5));
    check("v[3]", vpi_handle_by_index(v, 3));
    check("v[4]", vpi_handle_by_index(v, 4));
    PLI_INT32 good[2] = {1, 2};
    PLI_INT32 bad[2] = {2, 0};
    check("grid[1][2]", vpi_handle_by_multi_index(grid, 2, good));
    check("grid[2][0]", vpi_handle_by_multi_index(grid, 2, bad));
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
    reg(vpiSysTask, 0, "$d7", d7, NULL, NULL);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
