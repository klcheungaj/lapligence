/* Decision A19-D29: programs, program arrays, packages and `$unit` as VPI
 * instances (IEEE 1800-2009 37.9-37.11). See
 * A19-D29_programs_and_packages.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle get(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static const char* text(int property, vpiHandle h) {
    const char* s = h ? vpi_get_str(property, h) : NULL;
    return level() || !s ? "-" : s;
}

static void object(const char* label, vpiHandle h) {
    int error = level();
    if (!h)
        vpi_printf("%s NULL level=%d\n", label, error);
    else
        vpi_printf("%s type=%d %s level=%d\n", label, (int)vpi_get(vpiType, h),
                   text(vpiFullName, h), error);
}

static void relation(const char* label, int type, vpiHandle h) {
    vpiHandle r = vpi_handle(type, h);
    object(label, r);
}

static void property(const char* label, int property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    int error = level();
    if (error)
        vpi_printf("%s error=%d\n", label, error);
    else
        vpi_printf("%s %d\n", label, value);
}

static void iteration(const char* label, int type, vpiHandle h) {
    vpiHandle it = vpi_iterate(type, h), x;
    int error = level();
    vpi_printf("%s", label);
    if (!it) vpi_printf(" NULL");
    while (it && (x = vpi_scan(it)))
        vpi_printf(" %d:%s", (int)vpi_get(vpiType, x), text(vpiFullName, x));
    vpi_printf(" level=%d\n", error);
}

static long value_of(vpiHandle h) {
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(h, &value);
    return (long)value.value.integer;
}

static PLI_INT32 d29(PLI_BYTE8* data) {
    (void)data;
    iteration("top instances", vpiInstance, NULL);
    iteration("top packages", vpiPackage, NULL);
    iteration("top programs", vpiProgram, NULL);
    vpiHandle pkg = get("pkg");
    object("pkg", pkg);
    vpi_printf("pkg name=%s defname=%s\n", text(vpiName, pkg), text(vpiDefName, pkg));
    property("pkg vpiUnit", vpiUnit, pkg);
    relation("pkg vpiInstance", vpiInstance, pkg);
    relation("pkg vpiParent", vpiParent, pkg);
    iteration("pkg variables", vpiVariables, pkg);
    iteration("pkg taskfuncs", vpiTaskFunc, pkg);
    iteration("pkg ports", vpiPort, pkg);
    vpiHandle pv = get("pkg::pv");
    object("pkg::pv", pv);
    vpiHandle scoped = vpi_handle_by_name((PLI_BYTE8*)"pv", pkg);
    vpi_printf("pv by scope same=%d\n", scoped ? (int)vpi_compare_objects(scoped, pv) : -1);
    relation("pkg::pv vpiInstance", vpiInstance, pv);
    relation("pkg::pv vpiScope", vpiScope, pv);
    vpiHandle unit = NULL, it = vpi_iterate(vpiPackage, NULL), x;
    while (it && (x = vpi_scan(it)))
        if (vpi_get(vpiUnit, x) == 1) unit = x;
    object("unit", unit);
    property("unit vpiUnit", vpiUnit, unit);
    iteration("unit variables", vpiVariables, unit);
    object("$unit::uv by name", get("$unit::uv"));
    vpiHandle p = get("tb.p");
    object("tb.p", p);
    vpi_printf("tb.p defname=%s\n", text(vpiDefName, p));
    relation("tb.p vpiInstance", vpiInstance, p);
    relation("tb.p vpiModule", vpiModule, p);
    relation("tb.p vpiParent", vpiParent, p);
    relation("tb.p vpiIndex", vpiIndex, p);
    relation("tb.p vpiInstanceArray", vpiInstanceArray, p);
    property("tb.p vpiArrayMember", vpiArrayMember, p);
    relation("tb.p.x vpiInstance", vpiInstance, get("tb.p.x"));
    relation("tb.p.x vpiModule", vpiModule, get("tb.p.x"));
    iteration("tb programs", vpiProgram, get("tb"));
    iteration("tb program arrays", vpiProgramArray, get("tb"));
    vpiHandle pa = get("tb.pa");
    object("tb.pa", pa);
    property("tb.pa vpiSize", vpiSize, pa);
    iteration("tb.pa instances", vpiInstance, pa);
    vpiHandle e = get("tb.pa[1]");
    relation("tb.pa[1] vpiInstanceArray", vpiInstanceArray, e);
    property("tb.pa[1] vpiArrayMember", vpiArrayMember, e);
    vpiHandle index = vpi_handle(vpiIndex, e);
    vpi_printf("tb.pa[1] vpiIndex %ld level=%d\n", index ? value_of(index) : -1L, level());
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d29";
    task.calltf = d29;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
