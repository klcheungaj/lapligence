/* Companion of A20-D1_mcd_file_descriptor.sv: a VPI application (vlog_startup_routines). */
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

static PLI_INT32 d1(PLI_BYTE8* data) {
    (void)data;
    vpiHandle arguments = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    PLI_UINT32 fd = int_argument(arguments);
    vpi_release_handle(arguments);
    PLI_INT32 written = vpi_mcd_printf(fd, (PLI_BYTE8*)"vpi line\n");
    PLI_INT32 written_level = vpi_chk_error(NULL);
    PLI_INT32 flushed = vpi_mcd_flush(fd);
    PLI_INT32 flushed_level = vpi_chk_error(NULL);
    PLI_UINT32 unclosed = vpi_mcd_close(fd);
    PLI_INT32 closed_level = vpi_chk_error(NULL);
    PLI_UINT32 channel = vpi_mcd_open((PLI_BYTE8*)"a20_d1.txt");
    PLI_INT32 open_level = vpi_chk_error(NULL);
    vpi_printf("fd: bit31=%u\n", (unsigned)(fd >> 31));
    vpi_printf("printf: %s level=%d\n", written == EOF ? "EOF" : "written", (int)written_level);
    vpi_printf("flush: %s level=%d\n", flushed ? "nonzero" : "0", (int)flushed_level);
    vpi_printf("close: %s level=%d\n", unclosed == fd ? "unclosed" : "other", (int)closed_level);
    vpi_printf("open: %s level=%d\n", channel == 0 ? "0" : (channel == fd ? "fd" : "channel"),
               (int)open_level);
    if (channel != 0 && channel != fd) vpi_mcd_close(channel);
    return 0;
}

static void register_all(void) { reg("$d1", d1); }

void (*vlog_startup_routines[])(void) = {register_all, NULL};
