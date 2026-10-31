/* Decision A19-D58: the connections of a port declared with a module-side
 * concatenation (IEEE 1800-2009 23.2.2.1, 37.14). Companion of A19-D58_concatenation_ports.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>
static void expr(const char* label, vpiHandle h) {
    s_vpi_error_info e;
    int err = vpi_chk_error(&e);
    if (!h) { vpi_printf("    %s NULL%s\n", label, err ? "(err)" : ""); return; }
    s_vpi_value v; memset(&v, 0, sizeof v); v.format = vpiBinStrVal; vpi_get_value(h, &v);
    int verr = vpi_chk_error(&e);
    vpi_printf("    %s %s decompile=%s size=%d value=%s%s\n", label, (const char*)vpi_get_str(vpiType, h),
               (const char*)vpi_get_str(vpiDecompile, h), (int)vpi_get(vpiSize, h),
               verr ? "-" : v.value.str, verr ? "(err)" : "");
    if (vpi_get(vpiType, h) == vpiOperation) {
        vpi_printf("      op=%d\n", (int)vpi_get(vpiOpType, h));
        vpiHandle it = vpi_iterate(vpiOperand, h), o;
        while (it && (o = vpi_scan(it)))
            vpi_printf("      operand %s %s\n", (const char*)vpi_get_str(vpiType, o), (const char*)vpi_get_str(vpiFullName, o) ? (const char*)vpi_get_str(vpiFullName, o) : (const char*)vpi_get_str(vpiDecompile, o));
    }
}
static void inst(const char* name) {
    vpiHandle it = vpi_iterate(vpiPort, vpi_handle_by_name((PLI_BYTE8*)name, NULL)), p;
    while (it && (p = vpi_scan(it))) {
        vpi_printf("  port %s dir=%d size=%d index=%d explicit=%d\n", (const char*)vpi_get_str(vpiFullName, p),
                   (int)vpi_get(vpiDirection, p), (int)vpi_get(vpiSize, p), (int)vpi_get(vpiPortIndex, p),
                   (int)vpi_get(vpiExplicitName, p));
        expr("low", vpi_handle(vpiLowConn, p));
        expr("high", vpi_handle(vpiHighConn, p));
        vpiHandle bi = vpi_iterate(vpiBit, p), b;
        while (bi && (b = vpi_scan(bi))) {
            vpi_printf("    bit %s\n", (const char*)vpi_get_str(vpiFullName, b));
            expr(" blow", vpi_handle(vpiLowConn, b));
            expr(" bhigh", vpi_handle(vpiHighConn, b));
        }
    }
}
static PLI_INT32 d58(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("tb.s:\n"); inst("tb.s");
    vpi_printf("tb.t:\n"); inst("tb.t");
    /* Drivers of the high-side net through the port. */
    vpiHandle w = vpi_handle_by_name("tb.w", NULL);
    vpiHandle it = vpi_iterate(vpiDriver, w), d;
    while (it && (d = vpi_scan(it))) vpi_printf("  driver of tb.w: %s\n", (const char*)vpi_get_str(vpiType, d));
    return 0;
}
static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d58";
    task.calltf = d58;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, 0};
