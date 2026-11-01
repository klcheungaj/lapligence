/* Decision A19-D64: the value of a switch and the name of a class object.
 * Companion of A19-D64_switch_values_and_class_object_names.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failed(void) {
    s_vpi_error_info error;
    return vpi_chk_error(&error) != 0;
}

static PLI_INT32 d64(PLI_BYTE8* data) {
    (void)data;
    static const char* const switches[] = {"tb.sw1", "tb.sw0", "tb.sw2"};
    for (unsigned i = 0; i < sizeof switches / sizeof switches[0]; ++i) {
        vpiHandle sw = vpi_handle_by_name((PLI_BYTE8*)switches[i], NULL);
        s_vpi_value v;
        memset(&v, 0, sizeof v);
        v.format = vpiBinStrVal;
        vpi_get_value(sw, &v);
        int error = failed();
        vpi_printf("%s %s value=%s%s\n", (const char*)vpi_get_str(vpiType, sw), switches[i],
                   error ? "-" : v.value.str, error ? "(error)" : "");
    }
    static const char* const variables[] = {"tb.p", "tb.b"};
    for (unsigned i = 0; i < sizeof variables / sizeof variables[0]; ++i) {
        vpiHandle object = vpi_handle(vpiClassObj, vpi_handle_by_name((PLI_BYTE8*)variables[i], NULL));
        const char* name = vpi_get_str(vpiName, object);
        int error = failed();
        vpi_printf("%s object name=%s%s", variables[i], name ? name : "NULL", error ? "(error)" : "");
        const char* full = vpi_get_str(vpiFullName, object);
        error = failed();
        vpi_printf(" full=%s%s\n", full ? full : "NULL", error ? "(error)" : "");
    }
    return 0;
}

static void register_d64(void) {
    s_vpi_systf_data task = {vpiSysTask, 0, (PLI_BYTE8*)"$d64", d64, NULL, NULL, NULL};
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_d64, NULL};
