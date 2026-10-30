/* Decision A19-D28: index, range and select relations of nets and
 * variables (IEEE 1800-2009 37.16-37.18). See
 * A19-D28_index_and_packed_relations.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle get(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static long value_of(vpiHandle h) {
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(h, &value);
    return (long)value.value.integer;
}

static const char* name_of(vpiHandle h) {
    const char* s = h ? vpi_get_str(vpiFullName, h) : NULL;
    return s ? s : "NULL";
}

static void property(const char* label, int property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    int error = level();
    if (error)
        vpi_printf("%s error=%d\n", label, error);
    else
        vpi_printf("%s %d\n", label, value);
}

static void indices(const char* label, vpiHandle h) {
    vpiHandle one = vpi_handle(vpiIndex, h);
    int error = level();
    vpi_printf("%s vpiIndex %s level=%d iteration", label, one ? "handle" : "NULL", error);
    vpiHandle it = vpi_iterate(vpiIndex, h), x;
    error = level();
    if (!it) vpi_printf(" NULL");
    while (it && (x = vpi_scan(it))) vpi_printf(" %ld", value_of(x));
    vpi_printf(" level=%d\n", error);
}

static void ranges(const char* label, vpiHandle h) {
    vpiHandle it = vpi_iterate(vpiRange, h), x;
    int error = level();
    vpi_printf("%s ranges", label);
    if (!it) vpi_printf(" NULL");
    while (it && (x = vpi_scan(it)))
        vpi_printf(" [%ld:%ld]", value_of(vpi_handle(vpiLeftRange, x)),
                   value_of(vpi_handle(vpiRightRange, x)));
    vpi_printf(" level=%d\n", error);
}

static vpiHandle first_bit(vpiHandle h) {
    vpiHandle it = vpi_iterate(vpiBit, h);
    vpiHandle bit = it ? vpi_scan(it) : NULL;
    if (bit) vpi_release_handle(it);
    return bit;
}

static void relation(const char* label, int relation, vpiHandle h) {
    vpiHandle x = vpi_handle(relation, h);
    int error = level();
    vpi_printf("%s %s level=%d\n", label, name_of(x), error);
}

static PLI_INT32 d28(PLI_BYTE8* data) {
    (void)data;
    vpiHandle mem = get("tb.mem"), mem2 = get("tb.mem2");
    int index[2] = {1, 3};
    vpiHandle element = vpi_handle_by_index(mem, 1);
    vpiHandle element2 = vpi_handle_by_multi_index(mem2, 2, index);
    indices("tb.v", get("tb.v"));
    indices("tb.v bit", first_bit(get("tb.v")));
    indices("tb.mem[1]", element);
    indices("tb.mem2[1][3]", element2);
    indices("tb.mem2[1][3] bit", first_bit(element2));
    ranges("tb.v", get("tb.v"));
    ranges("tb.sc", get("tb.sc"));
    ranges("tb.pa", get("tb.pa"));
    ranges("tb.w", get("tb.w"));
    ranges("tb.mem[1]", element);
    property("tb.w vpiConstantSelect", vpiConstantSelect, get("tb.w"));
    property("tb.w bit vpiConstantSelect", vpiConstantSelect, first_bit(get("tb.w")));
    property("tb.v bit vpiConstantSelect", vpiConstantSelect, first_bit(get("tb.v")));
    property("tb.pa vpiConstantSelect", vpiConstantSelect, get("tb.pa"));
    property("tb.ps vpiConstantSelect", vpiConstantSelect, get("tb.ps"));
    property("tb.en vpiConstantSelect", vpiConstantSelect, get("tb.en"));
    property("tb.pa vpiPacked", vpiPacked, get("tb.pa"));
    property("tb.pa vpiPackedArrayMember", vpiPackedArrayMember, get("tb.pa"));
    property("tb.ps vpiPackedArrayMember", vpiPackedArrayMember, get("tb.ps"));
    vpiHandle it = vpi_iterate(vpiElement, get("tb.pa"));
    vpi_printf("tb.pa vpiElement %s level=%d\n", it ? "iterator" : "NULL", level());
    it = vpi_iterate(vpiVarSelect, mem);
    vpiHandle x;
    vpi_printf("tb.mem vpiVarSelect");
    while (it && (x = vpi_scan(it)))
        /* vpiReg and vpiLogicVar are one type (37.17 detail 19). */
        vpi_printf(" %d:%s", (int)vpi_get(vpiType, x), name_of(x));
    vpi_printf("\n");
    property("tb.rv vpiScalar", vpiScalar, get("tb.rv"));
    property("tb.rv vpiVector", vpiVector, get("tb.rv"));
    property("tb.s vpiScalar", vpiScalar, get("tb.s"));
    property("tb.s vpiVector", vpiVector, get("tb.s"));
    relation("tb.u[0] vpiInstanceArray", vpiInstanceArray, get("tb.u[0]"));
    relation("tb.u[0] vpiModuleArray", vpiModuleArray, get("tb.u[0]"));
    relation("tb.single vpiInstanceArray", vpiInstanceArray, get("tb.single"));
    relation("tb.g[1] vpiPrimitiveArray", vpiPrimitiveArray, get("tb.g[1]"));
    property("tb.w bit vpiNetType", vpiNetType, first_bit(get("tb.w")));
    it = vpi_iterate(vpiPort, get("tb.single"));
    vpiHandle port = it ? vpi_scan(it) : NULL;
    if (port) vpi_release_handle(it);
    property("tb.single port bit vpiPortType", vpiPortType, port ? first_bit(port) : NULL);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d28";
    task.calltf = d28;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
