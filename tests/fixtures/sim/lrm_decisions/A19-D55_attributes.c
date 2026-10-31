/* Decision A19-D55: attribute instances (IEEE 1800-2009 5.12, 37.78).
 * Companion of A19-D55_attributes.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>
/* Each attribute of `owner` with its natural value, binary value,
 * vpiDefAttribute, definition location, parent, full name and size. */
static void attrs(const char* what, vpiHandle owner) {
    s_vpi_error_info error;
    vpiHandle it = vpi_iterate(vpiAttribute, owner), a;
    vpi_printf("%s:\n", what);
    while (it && (a = vpi_scan(it))) {
        s_vpi_value value;
        memset(&value, 0, sizeof value);
        value.format = vpiObjTypeVal;
        vpi_get_value(a, &value);
        char text[80];
        switch (value.format) {
            case vpiIntVal: snprintf(text, sizeof text, "int:%d", (int)value.value.integer); break;
            case vpiRealVal: snprintf(text, sizeof text, "real:%g", value.value.real); break;
            case vpiStringVal: snprintf(text, sizeof text, "string:%s", value.value.str); break;
            case vpiScalarVal: snprintf(text, sizeof text, "scalar:%d", (int)value.value.scalar); break;
            default: snprintf(text, sizeof text, "format:%d", (int)value.format); break;
        }
        s_vpi_value bits;
        memset(&bits, 0, sizeof bits);
        bits.format = vpiBinStrVal;
        vpi_get_value(a, &bits);
        const char* file = (const char*)vpi_get_str(vpiDefFile, a);
        const char* base = file ? strrchr(file, '/') : NULL;
        vpiHandle parent = vpi_handle(vpiParent, a);
        const char* full = (const char*)vpi_get_str(vpiFullName, a);
        int size = (int)vpi_get(vpiSize, a);
        int size_failed = vpi_chk_error(&error);
        vpi_printf("  %s %s value=%s bits=%s def=%d at=%s:%d parent=%s full=%s size=%d%s\n",
                   (const char*)vpi_get_str(vpiName, a), (const char*)vpi_get_str(vpiType, a),
                   text, bits.value.str, (int)vpi_get(vpiDefAttribute, a),
                   base ? base + 1 : file ? file : "NULL", (int)vpi_get(vpiDefLineNo, a),
                   parent ? (const char*)vpi_get_str(vpiType, parent) : "NULL",
                   full ? full : "NULL", size, size_failed ? "(error)" : "");
    }
}
static PLI_INT32 d55(PLI_BYTE8* data) {
    (void)data;
    static const char* const owners[] = {"tb.s", "tb.t", "tb.s.w", "tb.s.v", "tb.s.e",
                                         "tb.s.f", "tb.s.b1", "tb.r", "tb.x"};
    for (size_t i = 0; i < sizeof owners / sizeof owners[0]; ++i)
        attrs(owners[i], vpi_handle_by_name((PLI_BYTE8*)owners[i], NULL));
    vpiHandle s = vpi_handle_by_name("tb.s", NULL);
    vpiHandle it = vpi_iterate(vpiPort, s), h;
    while (it && (h = vpi_scan(it))) attrs((const char*)vpi_get_str(vpiFullName, h), h);
    it = vpi_iterate(vpiProcess, s);
    while (it && (h = vpi_scan(it))) attrs("process", h);
    it = vpi_iterate(vpiContAssign, s);
    while (it && (h = vpi_scan(it))) attrs("contassign", h);
    /* An attribute is reached only from its owner. */
    vpi_printf("by name tb.s.keep: %s\n", vpi_handle_by_name("tb.s.keep", NULL) ? "found" : "NULL");
    return 0;
}
static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d55";
    task.calltf = d55;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, 0};
