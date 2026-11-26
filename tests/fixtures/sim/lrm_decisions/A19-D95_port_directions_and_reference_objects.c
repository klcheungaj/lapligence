#include "sv_vpi_user.h"
#include <string.h>

static int failed(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) != 0;
}

static void property(const char* label, PLI_INT32 prop, vpiHandle h) {
    PLI_INT32 value = vpi_get(prop, h);
    vpi_printf(" %s=%d%s", label, (int)value, failed() ? " (error)" : "");
}

static void text(const char* label, PLI_INT32 prop, vpiHandle h) {
    const char* value = vpi_get_str(prop, h);
    vpi_printf(" %s=%s%s", label, value ? value : "NULL", failed() ? " (error)" : "");
}

static void value(vpiHandle h, PLI_INT32 format) {
    s_vpi_value v;
    v.format = format;
    vpi_get_value(h, &v);
    if (failed()) {
        vpi_printf(" value=(error)");
        return;
    }
    if (format == vpiIntVal) vpi_printf(" value=%d", (int)v.value.integer);
    else if (format == vpiRealVal) vpi_printf(" value=%.2f", v.value.real);
    else vpi_printf(" value=%s", v.value.str);
}

static void related(const char* label, PLI_INT32 relation, vpiHandle h) {
    vpiHandle other = vpi_handle(relation, h);
    int error = failed();
    if (!other) {
        vpi_printf(" %s=NULL%s", label, error ? " (error)" : "");
        return;
    }
    vpi_printf(" %s=%d:%s", label, (int)vpi_get(vpiType, other),
               vpi_get_str(vpiFullName, other) ? vpi_get_str(vpiFullName, other) : "NULL");
}

static void reference(vpiHandle ref, PLI_INT32 format) {
    vpi_printf("    ref:");
    property("type", vpiType, ref);
    text("name", vpiName, ref);
    text("full", vpiFullName, ref);
    property("size", vpiSize, ref);
    property("dir", vpiDirection, ref);
    property("virtual", vpiVirtual, ref);
    property("generic", vpiGeneric, ref);
    related("actual", vpiActual, ref);
    related("parent", vpiParent, ref);
    vpiHandle ts = vpi_handle(vpiTypespec, ref);
    (void)failed();
    vpi_printf(" typespec=%d", ts ? (int)vpi_get(vpiType, ts) : 0);
    value(ref, format);
    vpi_printf("\n");
}

static void ports(const char* scope) {
    vpiHandle module = vpi_handle_by_name((PLI_BYTE8*)scope, NULL);
    vpiHandle it = vpi_iterate(vpiPort, module), port;
    while (it && (port = vpi_scan(it)) != NULL) {
        vpi_printf("port");
        text("name", vpiName, port);
        property("dir", vpiDirection, port);
        property("ptype", vpiPortType, port);
        vpiHandle low = vpi_handle(vpiLowConn, port);
        (void)failed();
        if (low) {
            property("low", vpiType, low);
            property("lowsize", vpiSize, low);
        } else {
            vpi_printf(" low=NULL");
        }
        vpiHandle bits = vpi_iterate(vpiBit, port), bit;
        (void)failed();
        if (bits && (bit = vpi_scan(bits)) != NULL) {
            property("bitdir", vpiDirection, bit);
            vpi_free_object(bits);
        }
        vpi_printf("\n");
        if (low && vpi_get(vpiType, low) == vpiRefObj && vpi_get(vpiPortType, port) == vpiPort) {
            const char* name = vpi_get_str(vpiName, port);
            PLI_INT32 format = !strcmp(name, "rr") ? vpiRealVal
                               : !strcmp(name, "rs") ? vpiStringVal : vpiIntVal;
            reference(low, format);
        }
    }
    it = vpi_iterate(vpiIODecl, module);
    while (it && (port = vpi_scan(it)) != NULL) {
        vpi_printf("io");
        text("name", vpiName, port);
        property("dir", vpiDirection, port);
        property("size", vpiSize, port);
        vpiHandle expr = vpi_handle(vpiExpr, port);
        (void)failed();
        vpi_printf(" expr=%d\n", expr ? (int)vpi_get(vpiType, expr) : 0);
    }
}

static void variable(const char* name) {
    vpiHandle h = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpi_printf("%s:", name);
    property("type", vpiType, h);
    property("size", vpiSize, h);
    property("packed", vpiPacked, h);
    property("constsel", vpiConstantSelect, h);
    vpi_printf("\n");
}

static void put(const char* port_name, s_vpi_value* v) {
    vpiHandle module = vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL);
    vpiHandle it = vpi_iterate(vpiPort, module), port;
    while (it && (port = vpi_scan(it)) != NULL) {
        if (strcmp(vpi_get_str(vpiName, port), port_name)) continue;
        vpi_put_value(vpi_handle(vpiLowConn, port), v, NULL, vpiNoDelay);
        vpi_printf("put %s%s\n", port_name, failed() ? " (error)" : "");
        vpi_free_object(it);
        return;
    }
}

static PLI_INT32 d95(PLI_BYTE8* data) {
    (void)data;
    ports("tb.u");
    ports("tb.n");
    variable("tb.pk");
    variable("tb.pb");
    variable("tb.ua[1]");
    variable("tb.sa");
    s_vpi_value v;
    v.format = vpiIntVal;
    v.value.integer = 42;
    put("rf", &v);
    v.value.integer = 0x5a;
    put("rl", &v);
    v.format = vpiRealVal;
    v.value.real = 1.25;
    put("rr", &v);
    v.format = vpiStringVal;
    v.value.str = (PLI_BYTE8*)"xyz";
    put("rs", &v);
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d95";
    task.calltf = d95;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, NULL};
