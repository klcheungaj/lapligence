/* Companion of A20-D3_mcd_printf_channels.sv: a VPI application (vlog_startup_routines). */
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

static void write_to(const char* label, PLI_UINT32 mcd, const char* text) {
    PLI_INT32 written = vpi_mcd_printf(mcd, (PLI_BYTE8*)"%s", text);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("%s: %d level=%d\n", label, (int)written, (int)level);
}

static PLI_INT32 d3(PLI_BYTE8* data) {
    (void)data;
    vpiHandle arguments = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    PLI_UINT32 a = int_argument(arguments);
    PLI_UINT32 b = int_argument(arguments);
    vpi_release_handle(arguments);
    /* The highest channel bit: no other file is open in this case. */
    PLI_UINT32 unopened = UINT32_C(1) << 30;
    write_to("a", a, "one\n");
    write_to("a|b", a | b, "both\n");
    write_to("a|unopened", a | unopened, "part\n");
    write_to("unopened", unopened, "none\n");
    return 0;
}

static void register_all(void) { reg("$d3", d3); }

void (*vlog_startup_routines[])(void) = {register_all, NULL};
