/* Companion of A19-D1_enum_string_value.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static void show(const char* label, vpiHandle h) {
    s_vpi_value value;
    value.format = vpiStringVal;
    vpi_get_value(h, &value);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("%s: string=\"%s\" level=%d\n", label, level ? "" : value.value.str, (int)level);
}

static PLI_INT32 d1(PLI_BYTE8* data) {
    (void)data;
    vpiHandle w = vpi_handle_by_name("tb.w", NULL);
    show("w", w);
    vpiHandle constants = vpi_iterate(vpiEnumConst, vpi_handle(vpiTypespec, w));
    vpiHandle constant;
    while ((constant = vpi_scan(constants))) {
        const char* name = vpi_get_str(vpiName, constant);
        show(name ? name : "NULL", constant);
    }
    return 0;
}

static void startup(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = "$d1";
    data.calltf = d1;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
