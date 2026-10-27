/* Companion of A19-D9_modpath_objects.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static const char* name_of(vpiHandle h) {
    const char* name = h ? (const char*)vpi_get_str(vpiFullName, h) : NULL;
    return name ? name : "NULL";
}

static void terms(PLI_INT32 type, const char* label, vpiHandle path) {
    vpiHandle it = vpi_iterate(type, path), term;
    while (it && (term = vpi_scan(it)))
        vpi_printf(" %s(%s dir=%d edge=0x%02x)", label, name_of(vpi_handle(vpiExpr, term)),
                   (int)vpi_get(vpiDirection, term), (int)vpi_get(vpiEdge, term));
}

static PLI_INT32 d9(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiModPath, vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL)), path;
    int index = 0;
    while (it && (path = vpi_scan(it))) {
        const char* name = (const char*)vpi_get_str(vpiName, path);
        vpi_printf("path %d: name=%s type=%d polarity=%d data_polarity=%d has_ifnone=%d", index++,
                   name ? name : "NULL", (int)vpi_get(vpiPathType, path),
                   (int)vpi_get(vpiPolarity, path), (int)vpi_get(vpiDataPolarity, path),
                   (int)vpi_get(vpiModPathHasIfNone, path));
        terms(vpiModPathIn, "in", path);
        terms(vpiModPathOut, "out", path);
        vpiHandle data_term = vpi_handle(vpiModDataPathIn, path);
        vpi_printf(" data=%s", data_term ? name_of(vpi_handle(vpiExpr, data_term)) : "NULL");
        vpi_printf(" condition=%s\n", name_of(vpi_handle(vpiCondition, path)));
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d9";
    task.calltf = d9;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
