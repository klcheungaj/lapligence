/* Companion of A19-D89_structure_members.sv: a VPI application (vlog_startup_routines). */
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
    vpi_printf("%s: %s%s\n", name, h ? "found" : "NULL", failed ? " (error)" : "");
    return h;
}

static void members(const char* label, vpiHandle h) {
    vpiHandle it = h ? vpi_iterate(vpiMember, h) : NULL;
    int failed = error();
    vpiHandle m;
    vpi_printf("%s members:", label);
    while (it && (m = vpi_scan(it))) vpi_printf(" %s", str(vpiName, m));
    vpi_printf("%s\n", failed ? " (error)" : "");
}

static PLI_INT32 changed(p_cb_data data) {
    vpi_printf("callback %s=%d\n", str(vpiFullName, data->obj), (int)data->value->value.integer);
    return 0;
}

static PLI_INT32 calltf(PLI_BYTE8* user) {
    (void)user;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = args ? vpi_scan(args) : NULL;
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    if (arg) vpi_get_value(arg, &value);
    if (args && arg) vpi_free_object(args);
    int step = (int)value.value.integer;
    vpi_printf("== d89 %d\n", step);
    if (step == 1) {
        members("tb.r", find("tb.r"));
        vpiHandle arr = find("tb.r.arr");
        vpiHandle cell = find("tb.r.arr[1]");
        vpi_printf("arr member=%d cell member=%d\n", arr ? (int)vpi_get(vpiStructUnionMember, arr) : -1,
                   cell ? (int)vpi_get(vpiStructUnionMember, cell) : -1);
        static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
        static s_vpi_value format = {vpiIntVal, {0}};
        s_cb_data cb;
        memset(&cb, 0, sizeof cb);
        cb.reason = cbValueChange;
        cb.cb_rtn = changed;
        cb.obj = cell;
        cb.time = &time;
        cb.value = &format;
        vpiHandle registered = cell ? vpi_register_cb(&cb) : NULL;
        vpi_printf("callback on tb.r.arr[1]: %s%s\n", registered ? "registered" : "NULL",
                   error() ? " (error)" : "");
        vpiHandle c = find("tb.c");
        vpiHandle object = c ? vpi_handle(vpiClassObj, c) : NULL;
        vpiHandle properties = object ? vpi_iterate(vpiVariables, object) : NULL;
        vpiHandle property;
        while (properties && (property = vpi_scan(properties)))
            if (strcmp(str(vpiName, property), "pr") == 0) members("c.pr", property);
        (void)find("tb.r.sa[0]");
        (void)find("tb.ea[1].k[0]");
        (void)find("tb.c.pe.k");
    } else {
        memset(&value, 0, sizeof value);
        value.format = vpiIntVal;
        value.value.integer = 9;
        vpiHandle cell = find("tb.r.arr[1]");
        if (cell) vpi_put_value(cell, &value, NULL, vpiNoDelay);
        value.value.integer = 7;
        vpiHandle k = find("tb.c.pr.k");
        if (k) vpi_put_value(k, &value, NULL, vpiNoDelay);
    }
    return 0;
}

static void register_d89(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$d89";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_d89, 0};
