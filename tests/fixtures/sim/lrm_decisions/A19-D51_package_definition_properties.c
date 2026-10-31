/* Decision A19-D51: the definition properties of a package.
 * Companion of A19-D51_package_definition_properties.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>
/* The definition properties of design element `name`, each with whether
 * the query reported a vpiError; file names without their directory. */
static void show(const char* name) {
    vpiHandle p = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpi_printf("%s handle=%s\n", name, p ? "yes" : "NULL");
    if (!p) return;
    static const struct { int prop; const char* name; } ints[] = {
        {vpiCellInstance, "cell"}, {vpiDefNetType, "defnettype"}, {vpiDefLineNo, "defline"},
        {vpiDefDelayMode, "delaymode"}, {vpiUnconnDrive, "unconn"}, {vpiAutomatic, "automatic"},
        {vpiTop, "top"}, {vpiProtected, "protected"}, {vpiUnit, "unit"}, {vpiLineNo, "line"},
        {vpiTimeUnit, "timeunit"}, {vpiTimePrecision, "timeprec"}};
    for (unsigned i = 0; i < sizeof ints / sizeof ints[0]; ++i) {
        s_vpi_error_info e;
        PLI_INT32 v = vpi_get(ints[i].prop, p);
        int err = vpi_chk_error(&e);
        vpi_printf("  %s=%d%s\n", ints[i].name, (int)v, err ? " (error)" : "");
    }
    static const struct { int prop; const char* name; } strs[] = {
        {vpiDefFile, "deffile"}, {vpiLibrary, "library"}, {vpiCell, "cellname"}, {vpiConfig, "config"},
        {vpiDefName, "defname"}, {vpiName, "name"}, {vpiFullName, "fullname"}};
    for (unsigned i = 0; i < sizeof strs / sizeof strs[0]; ++i) {
        s_vpi_error_info e;
        PLI_BYTE8* v = vpi_get_str(strs[i].prop, p);
        int err = vpi_chk_error(&e);
        const char* base = v ? strrchr((const char*)v, '/') : NULL;
        vpi_printf("  %s=%s%s\n", strs[i].name, v ? (base ? base + 1 : (const char*)v) : "NULL",
                   err ? " (error)" : "");
    }
}
static PLI_INT32 d51(PLI_BYTE8* data) {
    (void)data;
    show("pk");
    show("pa");
    show("tb");
    return 0;
}
static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d51";
    task.calltf = d51;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, 0};
