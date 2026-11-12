/* Companion of A19-D88_catalog_gaps.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int error(void) { return vpi_chk_error(NULL) != 0; }

static const char* str(int property, vpiHandle h) {
    static char buffers[4][256];
    static int next;
    char* buffer = buffers[next++ & 3];
    const char* text = h ? vpi_get_str(property, h) : NULL;
    snprintf(buffer, sizeof buffers[0], "%s", text ? text : "NULL");
    return buffer;
}

static vpiHandle find(const char* name) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    int failed = error();
    vpi_printf("%s: %s%s\n", name, h ? str(vpiFullName, h) : "NULL", failed ? " (error)" : "");
    return h;
}

static void list(const char* label, vpiHandle h, int type) {
    vpiHandle items = h ? vpi_iterate(type, h) : NULL;
    int failed = error();
    vpiHandle item;
    vpi_printf("%s:", label);
    while (items && (item = vpi_scan(items))) vpi_printf(" %s", str(vpiName, item));
    vpi_printf("%s\n", failed ? " (error)" : "");
}

static PLI_INT32 calltf(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = args ? vpi_scan(args) : NULL;
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    if (arg) vpi_get_value(arg, &value);
    if (args && arg) vpi_free_object(args);
    int step = (int)value.value.integer;
    vpi_printf("== d88 %d\n", step);
    vpiHandle frame = vpi_handle(vpiFrame, NULL);
    vpi_printf("frame: %s\n", str(vpiFullName, frame));
    list("frame automatics", frame, vpiAutomatics);
    vpiHandle x = find("tb.t.blk.x");
    if (x) {
        memset(&value, 0, sizeof value);
        value.format = vpiIntVal;
        vpi_get_value(x, &value);
        vpi_printf("x=%d name=%s\n", (int)value.value.integer, str(vpiName, x));
    }
    list("blk variables", find("tb.t.blk"), vpiVariables);
    if (step == 1) {
        vpiHandle m = find("p::C::m");
        vpiHandle body = m ? vpi_handle(vpiStmt, m) : NULL;
        vpi_printf("m stmt type=%d%s\n", body ? (int)vpi_get(vpiType, body) : 0,
                   error() ? " (error)" : "");
        vpiHandle nop = find("p::C::nop");
        body = nop ? vpi_handle(vpiStmt, nop) : NULL;
        vpi_printf("nop stmt %s%s\n", body ? "found" : "NULL", error() ? " (error)" : "");
    }
    return 0;
}

static void register_d88(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$d88";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_d88, 0};
