/* ADV-019 declaration and instance objects; see declarations.sv and the
 * readme. */
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
    if (error)
        vpi_printf("%s %s error=%d\n", label, name, error);
    else
        vpi_printf("%s %s %d\n", label, name, value);
}

static void text(const char* label, const char* name, int property, vpiHandle h) {
    const char* value = vpi_get_str(property, h);
    int error = level();
    const char* base = value;
    for (const char* p = value; p && *p; ++p)
        if (*p == '/' || *p == '\\') base = p + 1;
    if (error)
        vpi_printf("%s %s error=%d\n", label, name, error);
    else
        vpi_printf("%s %s %s\n", label, name, base ? base : "NULL");
}

/* 37.10, 33.7: the definition and binding of an instance. */
static void instance(const char* name) {
    vpiHandle h = get(name);
    text(name, "vpiConfig", vpiConfig, h);
    text(name, "vpiLibrary", vpiLibrary, h);
    text(name, "vpiCell", vpiCell, h);
    text(name, "vpiDefFile", vpiDefFile, h);
    integer(name, "vpiDefLineNo", vpiDefLineNo, h);
    integer(name, "vpiTop", vpiTop, h);
    integer(name, "vpiProtected", vpiProtected, h);
    integer(name, "vpiCellInstance", vpiCellInstance, h);
    integer(name, "vpiDefNetType", vpiDefNetType, h);
    integer(name, "vpiUnconnDrive", vpiUnconnDrive, h);
    integer(name, "vpiDefDelayMode", vpiDefDelayMode, h);
    integer(name, "vpiAutomatic", vpiAutomatic, h);
}

static PLI_INT32 declarations(PLI_BYTE8* data) {
    (void)data;
    instance("tb");
    instance("tb.u");
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$declarations";
    task.calltf = declarations;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
