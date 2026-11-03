/* Companion of A19-D81_assoc_string_key_names.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof info);
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

/* Look `name` up (relative to `scope` when given) and print what it names. */
static void look(const char* name, vpiHandle scope) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, scope);
    int error = level() != 0;
    s_vpi_value value;
    const char* full;
    memset(&value, 0, sizeof value);
    vpi_printf("%s%s ->", name, scope ? " in tb" : "");
    if (!h) {
        vpi_printf(" NULL error=%d\n", error);
        return;
    }
    full = vpi_get_str(vpiFullName, h);
    value.format = vpi_get(vpiType, h) == vpiStringVar ? vpiStringVal : vpiIntVal;
    vpi_get_value(h, &value);
    if (value.format == vpiStringVal)
        vpi_printf(" %s = %s\n", full ? full : "NULL", value.value.str ? value.value.str : "NULL");
    else
        vpi_printf(" %s = %d\n", full ? full : "NULL", (int)value.value.integer);
}

static PLI_INT32 d81(PLI_BYTE8* user_data) {
    vpiHandle aa = vpi_handle_by_name((PLI_BYTE8*)"tb.aa", NULL);
    vpiHandle it = aa ? vpi_iterate(vpiReg, aa) : NULL, x;
    (void)user_data;
    /* Every element's full name reads back. */
    while (it && (x = vpi_scan(it)) != NULL) {
        const char* full = vpi_get_str(vpiFullName, x);
        char copy[64];
        snprintf(copy, sizeof copy, "%s", full ? full : "NULL");
        look(copy, NULL);
    }
    look("tb.aa[\"zz\"]", NULL);
    look("tb.sa[\"k\"]", NULL);
    look("tb.sa[\"q\"]", NULL);
    look("aa[\"x\"]", vpi_handle_by_name((PLI_BYTE8*)"tb", NULL));
    return 0;
}

static void reg(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$d81";
    data.calltf = d81;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {reg, 0};
