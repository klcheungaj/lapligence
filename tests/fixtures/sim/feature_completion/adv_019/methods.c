/* ADV-019 class methods (see methods.sv): $methods(1) finds methods by name
 * and prints their statements. "!" marks an answer that raised an error. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failed(void) { return vpi_chk_error(NULL) != 0; }

static const char* str(int property, vpiHandle x) {
    static char buffers[4][256];
    static int next;
    char* buffer = buffers[next++ & 3];
    const char* s = vpi_get_str(property, x);
    snprintf(buffer, sizeof buffers[0], "%s%s", s ? s : "NULL", failed() ? "!" : "");
    return buffer;
}

static int get(int property, vpiHandle x) {
    int v = (int)vpi_get(property, x);
    return failed() ? -99 : v;
}

static void statement(vpiHandle s, int depth) {
    vpi_printf("%*sstmt type=%d line=%d\n", depth * 2, "", get(vpiType, s), get(vpiLineNo, s));
    if (depth > 3) return;
    vpiHandle inner = vpi_iterate(vpiStmt, s);
    vpiHandle k;
    while (inner && (k = vpi_scan(inner))) statement(k, depth + 1);
    (void)failed();
}

static void method(const char* label, vpiHandle m) {
    if (!m) {
        vpi_printf("%s: NULL%s\n", label, failed() ? "!" : "");
        return;
    }
    vpiHandle defn = vpi_handle(vpiClassDefn, m);
    vpi_printf("%s: type=%d name=%s full=%s line=%d defn=%s\n", label, get(vpiType, m),
               str(vpiName, m), str(vpiFullName, m), get(vpiLineNo, m),
               defn ? str(vpiName, defn) : "NULL");
    vpiHandle body = vpi_handle(vpiStmt, m);
    if (body)
        statement(body, 1);
    else
        vpi_printf("  stmt NULL%s\n", failed() ? "!" : "");
}

static vpiHandle by_name(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static PLI_INT32 calltf(PLI_BYTE8* data) {
    (void)data;
    method("tb.C::get", by_name("tb.C::get"));
    method("tb.C::run", by_name("tb.C::run"));
    method("tb.C::sf", by_name("tb.C::sf"));
    method("tb.C::nop", by_name("tb.C::nop"));
    method("tb.D::run", by_name("tb.D::run"));
    method("tb.D::get", by_name("tb.D::get"));
    method("tb.c.get", by_name("tb.c.get"));
    method("tb.d.run", by_name("tb.d.run"));
    method("tb.d.get", by_name("tb.d.get"));
    method("tb.c.missing", by_name("tb.c.missing"));
    method("tb.C::missing", by_name("tb.C::missing"));
    method("mpkg::PC::pget", by_name("mpkg::PC::pget"));
    vpiHandle c = by_name("tb.c");
    vpiHandle object = c ? vpi_handle(vpiClassObj, c) : NULL;
    vpiHandle methods = object ? vpi_iterate(vpiMethods, object) : NULL;
    vpiHandle m;
    while (methods && (m = vpi_scan(methods))) method("method of tb.c", m);
    return 0;
}

static void register_methods(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$methods";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_methods, 0};
