/* Decision A19-D53: the variable that holds a function's result.
 * Companion of A19-D53_function_return_variables.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

/* A string property, marked when the query reported a vpiError. */
static const char* text(int property, vpiHandle handle) {
    static char buffers[6][160];
    static int next;
    char* out = buffers[next++ % 6];
    s_vpi_error_info error;
    PLI_BYTE8* value = vpi_get_str(property, handle);
    int failed = vpi_chk_error(&error);
    snprintf(out, 160, "%s%s", value ? (const char*)value : "NULL", failed ? "(error)" : "");
    return out;
}

/* An integer property, marked when the query reported a vpiError. */
static const char* num(int property, vpiHandle handle) {
    static char buffers[6][40];
    static int next;
    char* out = buffers[next++ % 6];
    s_vpi_error_info error;
    PLI_INT32 value = vpi_get(property, handle);
    int failed = vpi_chk_error(&error);
    snprintf(out, 40, "%d%s", (int)value, failed ? "(error)" : "");
    return out;
}

/* The binary value of `handle`, or "-" when it has none. */
static const char* value(vpiHandle handle) {
    static char out[120];
    s_vpi_error_info error;
    s_vpi_value v;
    memset(&v, 0, sizeof v);
    v.format = vpiBinStrVal;
    vpi_get_value(handle, &v);
    int failed = vpi_chk_error(&error);
    snprintf(out, sizeof out, "%s", (!failed && v.value.str) ? v.value.str : "-");
    return out;
}

static PLI_INT32 d53(PLI_BYTE8* data) {
    (void)data;
    static const char* const names[] = {"sv_f", "af", "rng", "sf", "rf", "vf", "bit1", "tk"};
    for (unsigned i = 0; i < sizeof names / sizeof names[0]; ++i) {
        char full[40];
        snprintf(full, sizeof full, "tb.%s", names[i]);
        vpiHandle subroutine = vpi_handle_by_name(full, NULL);
        s_vpi_error_info error;
        vpiHandle result = vpi_handle(vpiReturn, subroutine);
        int failed = vpi_chk_error(&error);
        if (!result) {
            vpi_printf("%s return NULL%s\n", names[i], failed ? "(error)" : "");
            continue;
        }
        vpi_printf("%s return %s %s size=%s automatic=%s value=%s\n", names[i],
                   text(vpiType, result), text(vpiFullName, result), num(vpiSize, result),
                   num(vpiAutomatic, result), value(result));
        snprintf(full, sizeof full, "tb.%s.%s", names[i], names[i]);
        vpiHandle by_name = vpi_handle_by_name(full, NULL);
        vpi_printf("  by name same=%d scope=%s\n",
                   by_name ? (int)vpi_compare_objects(by_name, result) : -1,
                   text(vpiFullName, vpi_handle(vpiScope, result)));
    }
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d53";
    task.calltf = d53;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
