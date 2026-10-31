/* Decision A19-D57: the argument variables of static tasks and functions
 * (IEEE 1800-2009 13.3, 37.13, 37.17). Companion of A19-D57_static_subroutine_arguments.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>
static void value(const char* label, vpiHandle h) {
    s_vpi_error_info e;
    if (!h) { vpi_chk_error(&e); vpi_printf("  %s NULL\n", label); return; }
    s_vpi_value v; memset(&v, 0, sizeof v); v.format = vpiDecStrVal;
    vpi_get_value(h, &v);
    int err = vpi_chk_error(&e);
    vpi_printf("  %s %s %s size=%d auto=%d value=%s%s\n", label, (const char*)vpi_get_str(vpiType, h),
               (const char*)vpi_get_str(vpiFullName, h), (int)vpi_get(vpiSize, h), (int)vpi_get(vpiAutomatic, h),
               err ? "-" : v.value.str, err ? "(err)" : "");
}
static void scope(const char* name) {
    s_vpi_error_info e;
    vpiHandle s = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpi_printf("%s:\n", name);
    vpiHandle it = vpi_iterate(vpiVariables, s), h;
    while (it && (h = vpi_scan(it))) value("var", h);
    it = vpi_iterate(vpiIODecl, s);
    int err = vpi_chk_error(&e);
    if (err) vpi_printf("  iodecl iterate error: %s\n", e.message);
    while (it && (h = vpi_scan(it))) {
        vpi_printf("  io %s dir=%d\n", (const char*)vpi_get_str(vpiName, h), (int)vpi_get(vpiDirection, h));
        value("  expr", vpi_handle(vpiExpr, h));
    }
    static const char* const args[] = {"a", "b", "c", "loc", "x", "y", "q", "sf", "st"};
    for (size_t i = 0; i < sizeof args / sizeof args[0]; ++i) {
        char full[64];
        snprintf(full, sizeof full, "%s.%s", name, args[i]);
        vpiHandle h2 = vpi_handle_by_name(full, NULL);
        if (h2) value(full, h2);
    }
}
static PLI_INT32 d57(PLI_BYTE8* data) {
    (void)data;
    scope("tb.st");
    scope("tb.sf");
    scope("tb.at");
    /* Writing a static argument. */
    vpiHandle a = vpi_handle_by_name("tb.st.a", NULL);
    if (a) {
        s_vpi_value v; memset(&v, 0, sizeof v); v.format = vpiIntVal; v.value.integer = 42;
        vpi_put_value(a, &v, NULL, vpiNoDelay);
        s_vpi_error_info e; int err = vpi_chk_error(&e);
        value(err ? "after put(err)" : "after put", a);
    }
    return 0;
}
static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d57";
    task.calltf = d57;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, 0};
