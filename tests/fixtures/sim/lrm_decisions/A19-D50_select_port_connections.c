/* Decision A19-D50: the low connection of a port declared with a
 * module-side select. Companion of A19-D50_select_port_connections.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

/* One handle as type:full name, the strings copied before the next call
 * reuses the buffer they live in. A continuous assignment is named by its
 * target and a port bit by its port, neither having a name of its own. */
static const char* describe(vpiHandle handle) {
    static char buffers[4][160];
    static int next;
    char* out = buffers[next++ & 3];
    if (!handle) return "NULL";
    char type[40], name[96];
    const char* text = (const char*)vpi_get_str(vpiType, handle);
    snprintf(type, sizeof type, "%s", text ? text : "?");
    int kind = (int)vpi_get(vpiType, handle);
    vpiHandle named = kind == vpiContAssign ? vpi_handle(vpiLhs, handle)
                      : kind == vpiPortBit  ? vpi_handle(vpiParent, handle)
                                            : handle;
    text = named ? (const char*)vpi_get_str(vpiFullName, named) : NULL;
    snprintf(name, sizeof name, "%s", text ? text : "");
    snprintf(out, 160, "%s:%s", type, name);
    return out;
}

static const char* value(vpiHandle handle) {
    static char text[40];
    s_vpi_value v;
    memset(&v, 0, sizeof v);
    v.format = vpiBinStrVal;
    if (!handle) return "-";
    vpi_get_value(handle, &v);
    snprintf(text, sizeof text, "%s", v.value.str ? v.value.str : "?");
    return text;
}

static void list(const char* what, int relation, vpiHandle object) {
    vpiHandle it = vpi_iterate(relation, object), item;
    vpi_printf("%s:", what);
    if (!it) vpi_printf(" NULL");
    while (it && (item = vpi_scan(it))) vpi_printf(" %s", describe(item));
    vpi_printf("\n");
}

static PLI_INT32 d50(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiPort, vpi_handle_by_name("tb.u", NULL)), port;
    while (it && (port = vpi_scan(it))) {
        vpiHandle low = vpi_handle(vpiLowConn, port);
        vpi_printf("port %s low=%s", describe(port), describe(low));
        vpi_printf(" value=%s", value(low));
        vpi_printf(" high=%s\n", describe(vpi_handle(vpiHighConn, port)));
        vpiHandle bits = vpi_iterate(vpiBit, port), bit;
        while (bits && (bit = vpi_scan(bits))) {
            vpi_printf("  bit low=%s", describe(vpi_handle(vpiLowConn, bit)));
            vpi_printf(" high=%s\n", describe(vpi_handle(vpiHighConn, bit)));
        }
    }
    vpiHandle w = vpi_handle_by_name("tb.u.w", NULL);
    list("w drivers", vpiDriver, w);
    list("w local drivers", vpiLocalDriver, w);
    list("w ports", vpiPort, w);
    list("nv loads", vpiLoad, vpi_handle_by_name("tb.nv", NULL));
    list("nv portinst", vpiPortInst, vpi_handle_by_name("tb.nv", NULL));
    list("w[3] ports", vpiPort, vpi_handle_by_name("tb.u.w[3]", NULL));
    list("w[2] ports", vpiPort, vpi_handle_by_name("tb.u.w[2]", NULL));
    list("w[0] ports", vpiPort, vpi_handle_by_name("tb.u.w[0]", NULL));
    list("nv[1] portinst", vpiPortInst, vpi_handle_by_name("tb.nv[1]", NULL));
    list("nv[3] portinst", vpiPortInst, vpi_handle_by_name("tb.nv[3]", NULL));
    list("b drivers", vpiDriver, vpi_handle_by_name("tb.k.b", NULL));
    list("b ports", vpiPort, vpi_handle_by_name("tb.k.b", NULL));
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d50";
    task.calltf = d50;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
