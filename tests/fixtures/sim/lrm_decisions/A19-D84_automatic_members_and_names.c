/* Companion of A19-D84_automatic_members_and_names.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof info);
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static const char* str(int property, vpiHandle h) {
    const char* text = h ? vpi_get_str(property, h) : NULL;
    return text ? text : "NULL";
}

/* Own names: some simulators spell vpiLogicVar as vpiReg. */
static const char* type_name(int type) {
    switch (type) {
        case vpiLogicVar: return "logic";
        case vpiIntVar: return "int";
        case vpiStructVar: return "struct";
        default: return "other";
    }
}

/* A member: name, type, flags, parent and value, each printed before the
 * next call (vpi_get_str and value text may share a buffer). */
static void member(vpiHandle m) {
    s_vpi_value value;
    vpiHandle parent = vpi_handle(vpiParent, m);
    memset(&value, 0, sizeof value);
    vpi_printf("    %s", str(vpiName, m));
    vpi_printf(" %s", str(vpiFullName, m));
    vpi_printf(" %s size=%d struct_member=%d auto=%d", type_name((int)vpi_get(vpiType, m)),
               (int)vpi_get(vpiSize, m), (int)vpi_get(vpiStructUnionMember, m),
               (int)vpi_get(vpiAutomatic, m));
    vpi_printf(" parent=%s", str(vpiName, parent));
    value.format = vpiHexStrVal;
    vpi_get_value(m, &value);
    vpi_printf(" hex=%s\n", value.value.str ? value.value.str : "NULL");
}

static void put_int(vpiHandle m, int number) {
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    value.value.integer = number;
    vpi_put_value(m, &value, NULL, vpiNoDelay);
    vpi_printf("    put %s=%d error=%d\n", str(vpiFullName, m), number, level() != 0);
}

/* A structure variable or element found by name, and its members. */
static void structure(const char* name) {
    vpiHandle s = vpi_handle_by_name((PLI_BYTE8*)name, NULL), it, m;
    s_vpi_value value;
    if (!s) {
        vpi_printf("  %s NULL\n", name);
        return;
    }
    memset(&value, 0, sizeof value);
    vpi_printf("  %s %s size=%d", str(vpiFullName, s), type_name((int)vpi_get(vpiType, s)),
               (int)vpi_get(vpiSize, s));
    if (!vpi_get(vpiVector, s)) {
        value.format = vpiIntVal;
        vpi_get_value(s, &value);
        vpi_printf(" value error=%d", level() != 0);
    }
    vpi_printf("\n");
    it = vpi_iterate(vpiMember, s);
    while (it && (m = vpi_scan(it)) != NULL) member(m);
}

static void named(const char* name) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    s_vpi_value value;
    int error = level();
    memset(&value, 0, sizeof value);
    vpi_printf("  %s:", name);
    if (!h) {
        vpi_printf(" NULL error=%d\n", error != 0);
        return;
    }
    value.format = vpiIntVal;
    vpi_get_value(h, &value);
    vpi_printf(" %s value=%d\n", str(vpiFullName, h), (int)value.value.integer);
}

static void d84(int step) {
    vpiHandle h, m;
    vpi_printf("step %d\n", step);
    if (step == 0) {
        named("tb.t.rl");
        named("tb.rt.loc");
    } else if (step == 1) {
        structure("tb.t.rl");
        structure("tb.t.ri");
        structure("tb.t.ro");
        structure("tb.t.pl");
        structure("tb.t.pa[1]");
        h = vpi_handle_by_name((PLI_BYTE8*)"tb.t.rl", NULL);
        m = h ? vpi_handle_by_name((PLI_BYTE8*)"a", h) : NULL;
        vpi_printf("  a in tb.t.rl: %s\n", str(vpiFullName, m));
        if (m) put_int(m, 42);
        m = vpi_handle_by_name((PLI_BYTE8*)"tb.t.ro.a", NULL);
        if (m) put_int(m, 43);
        m = vpi_handle_by_name((PLI_BYTE8*)"tb.t.pl.h", NULL);
        if (m) put_int(m, 1);
        m = vpi_handle_by_name((PLI_BYTE8*)"tb.t.pa[1].l", NULL);
        if (m) put_int(m, 9);
        named("tb.t.ri.a");
    } else {
        named("tb.rt.n");
        named("tb.rt.loc");
    }
}

static PLI_INT32 calltf(PLI_BYTE8* user_data) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = args ? vpi_scan(args) : NULL;
    s_vpi_value value;
    (void)user_data;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    if (arg) vpi_get_value(arg, &value);
    if (args && arg) vpi_release_handle(args);
    d84((int)value.value.integer);
    return 0;
}

static void reg(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$d84";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {reg, 0};
