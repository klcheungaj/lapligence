/* Decision A19-D56: defparams (IEEE 1800-2009 23.10.1, 37.26, 37.80).
 * Companion of A19-D56_defparams.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>
static void show(const char* what, vpiHandle scope) {
    s_vpi_error_info e;
    vpi_chk_error(&e);
    vpiHandle it = vpi_iterate(vpiDefParam, scope), d;
    int err = vpi_chk_error(&e);
    vpi_printf("%s defparams%s:\n", what, err ? "(err)" : "");
    while (it && (d = vpi_scan(it))) {
        const char* type = (const char*)vpi_get_str(vpiType, d);
        int line = (int)vpi_get(vpiLineNo, d);
        const char* name = (const char*)vpi_get_str(vpiName, d);
        int nerr = vpi_chk_error(&e);
        const char* full = (const char*)vpi_get_str(vpiFullName, d);
        int ferr = vpi_chk_error(&e);
        vpiHandle mod = vpi_handle(vpiModule, d);
        int merr = vpi_chk_error(&e);
        vpiHandle par = vpi_handle(vpiParent, d);
        int perr = vpi_chk_error(&e);
        vpiHandle scp = vpi_handle(vpiScope, d);
        int serr = vpi_chk_error(&e);
        vpiHandle lhs = vpi_handle(vpiLhs, d);
        int lerr = vpi_chk_error(&e);
        vpiHandle rhs = vpi_handle(vpiRhs, d);
        int rerr = vpi_chk_error(&e);
        vpi_printf("  %s line=%d name=%s%s full=%s%s module=%s%s parent=%s%s scope=%s%s\n", type, line,
                   name ? name : "NULL", nerr ? "(err)" : "", full ? full : "NULL", ferr ? "(err)" : "",
                   mod ? (const char*)vpi_get_str(vpiFullName, mod) : "NULL", merr ? "(err)" : "",
                   par ? (const char*)vpi_get_str(vpiFullName, par) : "NULL", perr ? "(err)" : "",
                   scp ? (const char*)vpi_get_str(vpiFullName, scp) : "NULL", serr ? "(err)" : "");
        if (lhs) {
            s_vpi_value v; memset(&v, 0, sizeof v); v.format = vpiDecStrVal; vpi_get_value(lhs, &v);
            vpi_printf("    lhs %s %s value=%s\n", (const char*)vpi_get_str(vpiType, lhs),
                       (const char*)vpi_get_str(vpiFullName, lhs), v.value.str ? v.value.str : "-");
        } else vpi_printf("    lhs NULL%s\n", lerr ? "(err)" : "");
        if (rhs) {
            s_vpi_value v; memset(&v, 0, sizeof v); v.format = vpiDecStrVal; vpi_get_value(rhs, &v);
            int verr = vpi_chk_error(&e);
            vpi_printf("    rhs %s decompile=%s value=%s%s size=%d line=%d\n", (const char*)vpi_get_str(vpiType, rhs),
                       (const char*)vpi_get_str(vpiDecompile, rhs), v.value.str ? v.value.str : "-", verr ? "(err)" : "",
                       (int)vpi_get(vpiSize, rhs), (int)vpi_get(vpiLineNo, rhs));
            if (vpi_get(vpiType, rhs) == vpiOperation) {
                vpiHandle oi = vpi_iterate(vpiOperand, rhs), o;
                while (oi && (o = vpi_scan(oi)))
                    vpi_printf("      operand %s %s\n", (const char*)vpi_get_str(vpiType, o), (const char*)vpi_get_str(vpiDecompile, o));
            }
        } else vpi_printf("    rhs NULL%s\n", rerr ? "(err)" : "");
        vpi_chk_error(&e);
    }
}
static PLI_INT32 d56(PLI_BYTE8* data) {
    (void)data;
    show("tb", vpi_handle_by_name("tb", NULL));
    show("tb.t", vpi_handle_by_name("tb.t", NULL));
    show("tb.m", vpi_handle_by_name("tb.m", NULL));
    show("tb.g", vpi_handle_by_name("tb.g", NULL));
    show("tb.t.u", vpi_handle_by_name("tb.t.u", NULL));
    const char* ps[7] = {"tb.t.P", "tb.m.P", "tb.t.u.Q", "tb.m.u.Q", "tb.t.u.R", "tb.m.u.R", "tb.g.v.Q"};
    for (int i = 0; i < 7; ++i) {
        s_vpi_value v; memset(&v, 0, sizeof v); v.format = vpiDecStrVal;
        vpiHandle p = vpi_handle_by_name((PLI_BYTE8*)ps[i], NULL);
        if (p) vpi_get_value(p, &v);
        vpi_printf("%s = %s\n", ps[i], p && v.value.str ? v.value.str : "NULL");
    }
    return 0;
}
static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d56";
    task.calltf = d56;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, 0};
