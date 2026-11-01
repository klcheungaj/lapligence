/* Companion of A19-D72_virtual_interface_properties.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static void list(const char* label, vpiHandle owner) {
    vpiHandle it = vpi_iterate(vpiInterfaceDecl, owner), vi, actual;
    s_vpi_value value;
    int error = level() != 0;
    vpi_printf("%s error=%d\n", label, error);
    while (it && (vi = vpi_scan(it))) {
        vpi_printf("  %s type=%d virtual=%d", vpi_get_str(vpiName, vi), (int)vpi_get(vpiType, vi),
                   (int)vpi_get(vpiVirtual, vi));
        actual = vpi_handle(vpiActual, vi);
        error = level() != 0;
        vpi_printf(" actual=%s", actual ? vpi_get_str(vpiFullName, actual) : "NULL");
        vpi_printf(" actual_type=%d error=%d", actual ? (int)vpi_get(vpiType, actual) : 0, error);
        value.format = vpiObjTypeVal;
        vpi_get_value(vi, &value);
        vpi_printf(" value_error=%d\n", level() != 0);
    }
}

static PLI_INT32 d72(PLI_BYTE8* user) {
    vpiHandle k = vpi_handle_by_name((PLI_BYTE8*)"tb.k", NULL);
    vpiHandle object = vpi_handle(vpiClassObj, k);
    vpiHandle it = vpi_iterate(vpiClassDefn, NULL), definition = NULL, defn;
    (void)user;
    while (it && (defn = vpi_scan(it)))
        if (!definition && strcmp(vpi_get_str(vpiName, defn), "K") == 0) definition = defn;
    list("object", object);
    list("definition", definition);
    return 0;
}

static void register_d72(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof tf);
    tf.type = vpiSysTask;
    tf.tfname = (PLI_BYTE8*)"$d72";
    tf.calltf = d72;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {register_d72, 0};
