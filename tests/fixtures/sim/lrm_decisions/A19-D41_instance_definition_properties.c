/* Decision A19-D41: definition and configuration properties of instances
 * (IEEE 1800-2009 37.10, 33.7). See
 * A19-D41_instance_definition_properties.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static void integer(const char* label, const char* name, int property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    int error = level();
    vpi_printf("%s %s %d level=%d\n", label, name, value, error);
}

/* File names print without their directory. */
static void text(const char* label, const char* name, int property, vpiHandle h) {
    const char* value = vpi_get_str(property, h);
    int error = level();
    const char* base = value;
    for (const char* p = value; p && *p; ++p)
        if (*p == '/' || *p == '\\') base = p + 1;
    vpi_printf("%s %s %s level=%d\n", label, name, base ? base : "NULL", error);
}

static PLI_INT32 d41(PLI_BYTE8* data) {
    (void)data;
    const char* names[] = {"tb", "tb.c", "tb.n", "tb.t", "tb.am"};
    for (unsigned k = 0; k < sizeof names / sizeof names[0]; ++k) {
        vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)names[k], NULL);
        if (!h) {
            vpi_printf("%s missing\n", names[k]);
            continue;
        }
        integer(names[k], "vpiCellInstance", vpiCellInstance, h);
        integer(names[k], "vpiDefNetType", vpiDefNetType, h);
        integer(names[k], "vpiUnconnDrive", vpiUnconnDrive, h);
        integer(names[k], "vpiAutomatic", vpiAutomatic, h);
        integer(names[k], "vpiDefLineNo", vpiDefLineNo, h);
        text(names[k], "vpiDefFile", vpiDefFile, h);
        text(names[k], "vpiLibrary", vpiLibrary, h);
        text(names[k], "vpiCell", vpiCell, h);
        text(names[k], "vpiConfig", vpiConfig, h);
        integer(names[k], "vpiDefDelayMode", vpiDefDelayMode, h);
        integer(names[k], "vpiTop", vpiTop, h);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d41";
    task.calltf = d41;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
