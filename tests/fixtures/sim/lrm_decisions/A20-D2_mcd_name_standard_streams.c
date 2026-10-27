/* Companion of A20-D2_mcd_name_standard_streams.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static void reg(const char* name, PLI_INT32 (*calltf)(PLI_BYTE8*)) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)name;
    data.calltf = calltf;
    if (!vpi_register_systf(&data)) vpi_printf("registration of %s failed\n", name);
}

static PLI_UINT32 int_argument(vpiHandle arguments) {
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(arguments), &value);
    return (PLI_UINT32)value.value.integer;
}

static void show(const char* label, PLI_UINT32 cd) {
    PLI_BYTE8* name = vpi_mcd_name(cd);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("%s: %s level=%d\n", label, name ? (const char*)name : "NULL", (int)level);
}

static PLI_INT32 d2(PLI_BYTE8* data) {
    (void)data;
    vpiHandle arguments = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    PLI_UINT32 fd = int_argument(arguments);
    vpi_release_handle(arguments);
    show("mcd 1", 1);
    show("fd 0x80000000", 0x80000000u);
    show("fd 0x80000001", 0x80000001u);
    show("fd 0x80000002", 0x80000002u);
    show("$fopen fd", fd);
    return 0;
}

static void register_all(void) { reg("$d2", d2); }

void (*vlog_startup_routines[])(void) = {register_all, NULL};
