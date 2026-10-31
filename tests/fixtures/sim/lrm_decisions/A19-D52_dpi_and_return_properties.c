/* Decision A19-D52: the DPI properties and return range of tasks and
 * functions, and the interface of a modport.
 * Companion of A19-D52_dpi_and_return_properties.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

/* The DPI-C imports of the .sv file; the task only queries them. */
int cadd(int a, int b);
int cadd(int a, int b) { return a + b; }
void ctask(int a);
void ctask(int a) { (void)a; }
int d52_other(int a);
int d52_other(int a) { return a; }

/* An integer property, marked when the query reported a vpiError. */
static const char* num(int property, vpiHandle handle) {
    static char buffers[8][40];
    static int next;
    char* out = buffers[next++ & 7];
    s_vpi_error_info error;
    PLI_INT32 value = vpi_get(property, handle);
    int failed = vpi_chk_error(&error);
    snprintf(out, 40, "%d%s", (int)value, failed ? "(error)" : "");
    return out;
}

/* A string property, marked when the query reported a vpiError. */
static const char* text(int property, vpiHandle handle) {
    static char buffers[4][120];
    static int next;
    char* out = buffers[next++ & 3];
    s_vpi_error_info error;
    PLI_BYTE8* value = vpi_get_str(property, handle);
    int failed = vpi_chk_error(&error);
    snprintf(out, 120, "%s%s", value ? (const char*)value : "NULL", failed ? "(error)" : "");
    return out;
}

/* A range bound: its decimal value, or NULL. */
static const char* bound(int relation, vpiHandle handle) {
    static char buffers[2][60];
    static int next;
    char* out = buffers[next++ & 1];
    s_vpi_error_info error;
    vpiHandle range = vpi_handle(relation, handle);
    int failed = vpi_chk_error(&error);
    if (!range) {
        snprintf(out, 60, "NULL%s", failed ? "(error)" : "");
        return out;
    }
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    vpi_get_value(range, &value);
    snprintf(out, 60, "%s=%d", text(vpiType, range), (int)value.value.integer);
    return out;
}

static PLI_INT32 d52(PLI_BYTE8* data) {
    (void)data;
    vpiHandle modport = vpi_handle_by_name("tb.i0.mp", NULL);
    vpi_printf("modport %s interface %s\n", text(vpiFullName, modport),
               text(vpiFullName, vpi_handle(vpiInterface, modport)));
    static const char* const names[] = {"cadd", "ctask", "other", "sv_f", "sv_t",
                                        "rng",  "bit1",  "rf",    "vf"};
    for (unsigned i = 0; i < sizeof names / sizeof names[0]; ++i) {
        char full[40];
        snprintf(full, sizeof full, "tb.%s", names[i]);
        vpiHandle subroutine = vpi_handle_by_name(full, NULL);
        vpi_printf("%s %s access=%s pure=%s context=%s cstr=%s identifier=%s\n", names[i],
                   text(vpiType, subroutine), num(vpiAccessType, subroutine),
                   num(vpiDPIPure, subroutine), num(vpiDPIContext, subroutine),
                   num(vpiDPICStr, subroutine), text(vpiDPICIdentifier, subroutine));
        vpi_printf("  left=%s right=%s\n", bound(vpiLeftRange, subroutine),
                   bound(vpiRightRange, subroutine));
    }
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d52";
    task.calltf = d52;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
