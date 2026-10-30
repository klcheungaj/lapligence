/* Decision A19-D42: generate scope arrays, genvars, io declarations and
 * specparams (IEEE 1800-2009 37.13, 37.26, 37.80). See
 * A19-D42_declaration_objects.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle get(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static void integer(const char* label, const char* name, int property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    int error = level();
    vpi_printf("%s %s %d level=%d\n", label, name, value, error);
}

static void text(const char* label, const char* name, int property, vpiHandle h) {
    const char* value = vpi_get_str(property, h);
    int error = level();
    vpi_printf("%s %s %s level=%d\n", label, name, value ? value : "NULL", error);
}

static void relation(const char* label, const char* name, int type, vpiHandle h) {
    vpiHandle r = vpi_handle(type, h);
    int error = level();
    const char* full = r ? vpi_get_str(vpiFullName, r) : NULL;
    vpi_printf("%s %s %s type=%d level=%d\n", label, name, r ? (full ? full : "?") : "NULL",
               r ? (int)vpi_get(vpiType, r) : 0, error);
}

static void io_decls(const char* scope, vpiHandle h) {
    vpiHandle it = vpi_iterate(vpiIODecl, h);
    vpi_printf("%s vpiIODecl iterator=%s level=%d\n", scope, it ? "yes" : "NULL", level());
    for (vpiHandle io; it && (io = vpi_scan(it));) {
        const char* name = vpi_get_str(vpiName, io);
        integer(name, "vpiType", vpiType, io);
        integer(name, "vpiDirection", vpiDirection, io);
        integer(name, "vpiSize", vpiSize, io);
        integer(name, "vpiScalar", vpiScalar, io);
        integer(name, "vpiVector", vpiVector, io);
        integer(name, "vpiSigned", vpiSigned, io);
        text(name, "vpiFullName", vpiFullName, io);
        relation(name, "vpiExpr", vpiExpr, io);
        relation(name, "vpiTaskFunc", vpiTaskFunc, io);
        relation(name, "vpiModport", vpiModport, io);
        relation(name, "vpiModule", vpiModule, io);
        vpiHandle left = vpi_handle(vpiLeftRange, io);
        int error = level();
        s_vpi_value value = {vpiIntVal, {0}};
        if (left) vpi_get_value(left, &value);
        vpi_printf("%s vpiLeftRange %s %d level=%d\n", name, left ? "yes" : "NULL",
                   (int)value.value.integer, error);
        vpiHandle ranges = vpi_iterate(vpiRange, io);
        error = level();
        int count = 0;
        for (vpiHandle range; ranges && (range = vpi_scan(ranges));) ++count;
        vpi_printf("%s vpiRange %d level=%d\n", name, count, error);
    }
}

static PLI_INT32 d42(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = get("tb");
    vpiHandle it = vpi_iterate(vpiGenScopeArray, tb);
    for (vpiHandle array; it && (array = vpi_scan(it));) {
        text("array", "vpiFullName", vpiFullName, array);
        integer("array", "vpiSize", vpiSize, array);
        relation("array", "vpiGenVar", vpiGenVar, array);
        relation("array", "vpiModule", vpiModule, array);
        for (int i = 1; i <= 5; ++i) {
            vpiHandle scope = vpi_handle_by_index(array, i);
            int error = level();
            vpi_printf("array index %d %s level=%d\n", i,
                       scope ? vpi_get_str(vpiFullName, scope) : "NULL", error);
        }
        PLI_INT32 index = 3;
        vpiHandle multi = vpi_handle_by_multi_index(array, 1, &index);
        int error = level();
        vpi_printf("array multi index 3 %s level=%d\n",
                   multi ? vpi_get_str(vpiFullName, multi) : "NULL", error);
        vpiHandle scopes = vpi_iterate(vpiGenScope, array);
        for (vpiHandle scope; scopes && (scope = vpi_scan(scopes));)
            relation(vpi_get_str(vpiFullName, scope), "vpiGenScopeArray", vpiGenScopeArray, scope);
    }
    vpiHandle genvar = get("tb.g");
    integer("tb.g", "vpiType", vpiType, genvar);
    text("tb.g", "vpiFullName", vpiFullName, genvar);
    io_decls("tb.u", get("tb.u"));
    io_decls("tb.scale", get("tb.scale"));
    io_decls("tb.b.src", get("tb.b.src"));
    vpiHandle sp = vpi_iterate(vpiSpecParam, get("tb.u"));
    for (vpiHandle p; sp && (p = vpi_scan(sp));) {
        s_vpi_value value = {vpiIntVal, {0}};
        vpi_get_value(p, &value);
        vpi_printf("%s type=%d value=%d level=%d\n", vpi_get_str(vpiFullName, p),
                   (int)vpi_get(vpiType, p), (int)value.value.integer, level());
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d42";
    task.calltf = d42;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
