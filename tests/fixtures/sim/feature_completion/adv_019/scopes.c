/* ADV-019 family 3: dynamic objects outside instance scopes in scopes.sv
 * (IEEE 1800-2009 37.10, 37.17, 37.29): by name, their type, full name and
 * value or object; then the dynamic variables of package `dp` and of the
 * named block through vpiVariables. */
#include "sv_vpi_user.h"
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static void show(vpiHandle h) {
    const char* type = vpi_get_str(vpiType, h);
    const char* full = vpi_get_str(vpiFullName, h);
    vpi_printf("  %s %s", type ? type : "-", full ? full : "-");
    int t = (int)vpi_get(vpiType, h);
    if (t == vpiStringVar) {
        s_vpi_value v;
        v.format = vpiStringVal;
        vpi_get_value(h, &v);
        vpi_printf(" value=%s", v.value.str);
    } else if (t == vpiClassVar) {
        vpiHandle obj = vpi_handle(vpiClassObj, h);
        vpi_printf(" object id=%d", obj ? (int)vpi_get(vpiObjId, obj) : 0);
    } else if (t == vpiArrayVar) {
        vpi_printf(" size=%d", (int)vpi_get(vpiSize, h));
    }
    vpi_printf("\n");
}

static void by_name(const char* name) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    int status = level();
    if (!h) {
        vpi_printf("%s: null level=%d\n", name, status);
        return;
    }
    vpi_printf("%s:\n", name);
    show(h);
}

static void variables(const char* label, vpiHandle scope) {
    vpi_printf("variables of %s:\n", label);
    vpiHandle it = vpi_iterate(vpiVariables, scope), v;
    while (it && (v = vpi_scan(it))) show(v);
}

static PLI_INT32 walk(PLI_BYTE8* data) {
    (void)data;
    const char* names[] = {"dp::pobj", "dp::pname", "dp::pq", "dp::pq[1]", "tb.blk.label",
                           "tb.remember.held", "tb.keep.vif"};
    for (unsigned i = 0; i < sizeof names / sizeof names[0]; ++i) by_name(names[i]);
    variables("dp", vpi_handle_by_name((PLI_BYTE8*)"dp", NULL));
    variables("tb.blk", vpi_handle_by_name((PLI_BYTE8*)"tb.blk", NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$scopes";
    task.calltf = walk;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
