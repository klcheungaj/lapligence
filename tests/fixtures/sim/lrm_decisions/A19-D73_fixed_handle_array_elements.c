/* Companion of A19-D73_fixed_handle_array_elements.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static const char* name_of(vpiHandle h) {
    const char* name = h ? vpi_get_str(vpiFullName, h) : NULL;
    return name ? name : "NULL";
}

static void element(vpiHandle e) {
    vpiHandle parent, module, scope, index, it, object;
    s_vpi_value value;
    int error, count = 0;
    /* vpi_get_str reuses one buffer, so each string is printed at once. */
    vpi_printf("  %s", name_of(e));
    vpi_printf(" name=%s type=%d member=%d", vpi_get_str(vpiName, e), (int)vpi_get(vpiType, e),
               (int)vpi_get(vpiArrayMember, e));
    parent = vpi_handle(vpiParent, e);
    vpi_printf(" parent=%s", name_of(parent));
    module = vpi_handle(vpiModule, e);
    vpi_printf(" module=%s", name_of(module));
    scope = vpi_handle(vpiScope, e);
    vpi_printf(" scope=%s", name_of(scope));
    index = vpi_handle(vpiIndex, e);
    value.format = vpiIntVal;
    if (index) vpi_get_value(index, &value);
    vpi_printf(" index=%d", index ? (int)value.value.integer : -1);
    it = vpi_iterate(vpiIndex, e);
    while (it && vpi_scan(it)) ++count;
    vpi_printf(" indices=%d\n", count);
    if (vpi_get(vpiType, e) == vpiStringVar) {
        value.format = vpiStringVal;
        vpi_get_value(e, &value);
        vpi_printf("    value=\"%s\"\n", value.value.str);
    }
    if (vpi_get(vpiType, e) == vpiClassVar) {
        object = vpi_handle(vpiClassObj, e);
        error = level() != 0;
        vpi_printf("    object=%s error=%d\n", object ? "obj" : "NULL", error);
        if (object) {
            vpiHandle props = vpi_iterate(vpiVariables, object), p;
            while (props && (p = vpi_scan(props))) {
                value.format = vpiIntVal;
                vpi_get_value(p, &value);
                vpi_printf("    %s=%d\n", vpi_get_str(vpiName, p), (int)value.value.integer);
            }
        }
    }
}

static void array(const char* path, int low, int high) {
    vpiHandle a = vpi_handle_by_name((PLI_BYTE8*)path, NULL), it, e;
    int i, error;
    vpi_printf("%s type=%d size=%d\n", path, (int)vpi_get(vpiType, a), (int)vpi_get(vpiSize, a));
    it = vpi_iterate(vpiVarSelect, a);
    while (it && (e = vpi_scan(it))) element(e);
    for (i = low - 1; i <= high + 1; ++i) {
        e = vpi_handle_by_index(a, i);
        error = level() != 0;
        vpi_printf("  by_index %d = %s error=%d\n", i, name_of(e), error);
    }
}

static PLI_INT32 d73(PLI_BYTE8* user) {
    vpiHandle e;
    s_vpi_value value;
    (void)user;
    array("tb.sa", 0, 1);
    array("tb.ca", 2, 3);
    array("tb.cla", 0, 1);
    e = vpi_handle_by_name((PLI_BYTE8*)"tb.cla[1]", NULL);
    vpi_printf("by name tb.cla[1] = %s\n", name_of(e));
    e = vpi_handle_by_name((PLI_BYTE8*)"tb.sa[0]", NULL);
    value.format = vpiStringVal;
    value.value.str = (PLI_BYTE8*)"zz";
    vpi_put_value(e, &value, NULL, vpiNoDelay);
    vpi_printf("put error=%d\n", level() != 0);
    return 0;
}

static void register_d73(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof tf);
    tf.type = vpiSysTask;
    tf.tfname = (PLI_BYTE8*)"$d73";
    tf.calltf = d73;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {register_d73, 0};
