/* Companion of A19-D67_packed_array_elements.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle get(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static int bound(vpiHandle handle, PLI_INT32 relation) {
    vpiHandle constant = vpi_handle(relation, handle);
    s_vpi_value value;
    (void)level();
    if (!constant) return -99;
    value.format = vpiIntVal;
    vpi_get_value(constant, &value);
    return (int)value.value.integer;
}

static void show(const char* label, vpiHandle handle) {
    int error = level();
    s_vpi_value value;
    vpiHandle parent, iterator, item;
    int count = 0;
    if (!handle) {
        vpi_printf("%s: NULL error=%d\n", label, error != 0);
        return;
    }
    value.format = vpiHexStrVal;
    vpi_get_value(handle, &value);
    vpi_printf("%s: %s type=%d size=%d value=%s", label, vpi_get_str(vpiFullName, handle),
               (int)vpi_get(vpiType, handle), (int)vpi_get(vpiSize, handle), value.value.str);
    parent = vpi_handle(vpiParent, handle);
    vpi_printf(" parent=%s array=%d member=%d const=%d\n", parent ? vpi_get_str(vpiFullName, parent) : "NULL",
               (int)vpi_get(vpiArray, handle), (int)vpi_get(vpiArrayMember, handle),
               (int)vpi_get(vpiConstantSelect, handle));
    (void)level();
    vpi_printf("  index=%d left=%d right=%d", bound(handle, vpiIndex), bound(handle, vpiLeftRange),
               bound(handle, vpiRightRange));
    iterator = vpi_iterate(vpiBit, handle);
    (void)level();
    while (iterator && (item = vpi_scan(iterator))) {
        if (count++ == 0) vpi_printf(" first_bit=%s", vpi_get_str(vpiFullName, item));
    }
    vpi_printf(" bits=%d\n", count);
}

static void indices(const char* label, vpiHandle handle) {
    vpiHandle iterator = handle ? vpi_iterate(vpiIndex, handle) : NULL, item;
    s_vpi_value value;
    vpi_printf("%s indices:", label);
    while (iterator && (item = vpi_scan(iterator))) {
        value.format = vpiIntVal;
        vpi_get_value(item, &value);
        vpi_printf(" %d", (int)value.value.integer);
    }
    vpi_printf("\n");
    (void)level();
}

static void put(const char* label, vpiHandle handle, PLI_INT32 integer) {
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = integer;
    vpi_put_value(handle, &value, NULL, vpiNoDelay);
    vpi_printf("put %s error=%d\n", label, level() != 0);
}

static PLI_INT32 d67(PLI_BYTE8* data) {
    PLI_INT32 bit[2] = {2, 3}, element3[2] = {1, 2}, unpacked[2] = {1, 0}, out[2] = {2, 9};
    vpiHandle pa = get("tb.pa"), pb = get("tb.pb"), pn = get("tb.pn"), upa = get("tb.upa");
    vpiHandle ps = get("tb.ps"), element, again;
    (void)data;
    element = vpi_handle_by_index(pa, 2);
    show("pa by_index 2", element);
    again = get("tb.pa[2]");
    vpi_printf("compare=%d\n", (int)vpi_compare_objects(element, again));
    show("pa[2] by_index 3", vpi_handle_by_index(element, 3));
    show("pa multi {2,3}", vpi_handle_by_multi_index(pa, 2, bit));
    show("pa name [2][3]", get("tb.pa[2][3]"));
    show("pa by_index 9", vpi_handle_by_index(pa, 9));
    show("pa multi {2,9}", vpi_handle_by_multi_index(pa, 2, out));
    show("pb multi {1,2}", vpi_handle_by_multi_index(pb, 2, element3));
    show("pb by_index 1", vpi_handle_by_index(pb, 1));
    show("pn by_index 1", vpi_handle_by_index(pn, 1));
    show("upa multi {1,0}", vpi_handle_by_multi_index(upa, 2, unpacked));
    show("upa name [1][0][2]", get("tb.upa[1][0][2]"));
    show("ps by_index 0", vpi_handle_by_index(ps, 0));
    indices("pa[2]", element);
    indices("pa[2][3]", get("tb.pa[2][3]"));
    put("pa[2]", element, 0xaa);
    put("pa[1][0]", get("tb.pa[1][0]"), 1);
    put("pb[0][1]", get("tb.pb[0][1]"), 3);
    put("upa[0][1]", get("tb.upa[0][1]"), 7);
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d67";
    task.calltf = d67;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, 0};
