/* Companion of A19-D25_string_int_put.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static void put(vpiHandle s, PLI_INT32 value) {
    s_vpi_value v;
    v.format = vpiIntVal;
    v.value.integer = value;
    vpi_put_value(s, &v, NULL, vpiNoDelay);
    int level = vpi_chk_error(NULL);
    v.format = vpiStringVal;
    vpi_get_value(s, &v);
    vpi_printf("put 0x%08x level=%d -> \"%s\" size=%d\n", (unsigned)value, level, v.value.str,
               (int)vpi_get(vpiSize, s));
}

static PLI_INT32 d25(PLI_BYTE8* data) {
    (void)data;
    vpiHandle s = vpi_handle_by_name("tb.s", NULL);
    put(s, 0x43);
    put(s, 0x00414200);
    put(s, 0x41424344);
    put(s, 0);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d25";
    task.calltf = d25;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
