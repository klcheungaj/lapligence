/* Decision A19-D60: the typespecs of dynamic arrays, queues and associative
 * arrays (IEEE 1800-2009 37.23). Companion of A19-D60_container_typespecs.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>
static s_vpi_error_info e;
static void bound(const char* label, vpiHandle h) {
    if (!h) {
        int err = vpi_chk_error(&e);
        vpi_printf(" %s=NULL%s", label, err ? "(err)" : "");
        return;
    }
    const char* text = (const char*)vpi_get_str(vpiDecompile, h);
    vpi_chk_error(&e);
    vpi_printf(" %s=%s", label, text ? text : "?");
}
static void typespec(const char* label, vpiHandle t, int depth) {
    int err = vpi_chk_error(&e);
    if (!t) {
        vpi_printf("%*s%s NULL%s\n", 2 * depth, "", label, err ? "(err)" : "");
        return;
    }
    int type = (int)vpi_get(vpiType, t);
    const char* name = (const char*)vpi_get_str(vpiName, t);
    vpi_chk_error(&e);
    vpi_printf("%*s%s %s name=%s", 2 * depth, "", label, (const char*)vpi_get_str(vpiType, t),
               name ? name : "-");
    if (type == vpiArrayTypespec) {
        vpi_printf(" arraytype=%d", (int)vpi_get(vpiArrayType, t));
        vpiHandle left = vpi_handle(vpiLeftRange, t);
        bound("left", left);
        vpiHandle right = vpi_handle(vpiRightRange, t);
        bound("right", right);
    }
    vpi_printf("\n");
    if (type == vpiArrayTypespec || type == vpiLogicTypespec || type == vpiBitTypespec) {
        vpiHandle it = vpi_iterate(vpiRange, t), r;
        vpi_chk_error(&e);
        while (it && (r = vpi_scan(it))) {
            vpi_printf("%*s  range", 2 * depth, "");
            vpiHandle left = vpi_handle(vpiLeftRange, r);
            bound("left", left);
            vpiHandle right = vpi_handle(vpiRightRange, r);
            bound("right", right);
            vpi_printf("\n");
        }
    }
    if (type == vpiArrayTypespec) {
        vpiHandle index = vpi_handle(vpiIndexTypespec, t);
        typespec("index", index, depth + 1);
        vpiHandle element = vpi_handle(vpiElemTypespec, t);
        typespec("element", element, depth + 1);
    }
    if (type == vpiStructTypespec) {
        vpiHandle it = vpi_iterate(vpiTypespecMember, t), m;
        vpi_chk_error(&e);
        while (it && (m = vpi_scan(it))) {
            vpi_printf("%*s  member %s\n", 2 * depth, "", (const char*)vpi_get_str(vpiName, m));
            vpiHandle member = vpi_handle(vpiTypespec, m);
            typespec("typespec", member, depth + 2);
        }
    }
}
static void variable(const char* name) {
    vpiHandle v = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpi_chk_error(&e);
    vpi_printf("%s %s\n", name, v ? (const char*)vpi_get_str(vpiType, v) : "NULL");
    if (!v) return;
    vpiHandle t = vpi_handle(vpiTypespec, v);
    typespec("typespec", t, 1);
}
static PLI_INT32 d60(PLI_BYTE8* data) {
    (void)data;
    const char* names[] = {"tb.q",  "tb.qb", "tb.d",     "tb.as", "tb.ai", "tb.al",   "tb.aw",
                           "tb.tq", "tb.ta", "tb.mixed", "tb.dd", "tb.sv", "tb.fx", "tb.q[0]"};
    for (unsigned i = 0; i < sizeof names / sizeof *names; ++i) variable(names[i]);
    return 0;
}
static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d60";
    task.calltf = d60;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, 0};
