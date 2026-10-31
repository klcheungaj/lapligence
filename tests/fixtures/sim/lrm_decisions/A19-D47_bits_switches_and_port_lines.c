/* Decision A19-D47: bits, pull primitives and port lines.
 * Companion of A19-D47_bits_switches_and_port_lines.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>

/* One handle as type:full name@line, the strings copied before the next
 * call reuses the buffer they live in. */
static const char* describe(vpiHandle handle) {
    static char buffers[4][160];
    static int next;
    char* out = buffers[next++ & 3];
    if (!handle) return "NULL";
    char type[40], name[96];
    const char* text = (const char*)vpi_get_str(vpiType, handle);
    snprintf(type, sizeof type, "%s", text ? text : "?");
    text = (const char*)vpi_get_str(vpiFullName, handle);
    snprintf(name, sizeof name, "%s", text ? text : "");
    snprintf(out, 160, "%s:%s@%d", type, name, (int)vpi_get(vpiLineNo, handle));
    return out;
}

static void bits(const char* what, vpiHandle object) {
    vpiHandle it = vpi_iterate(vpiBit, object), bit;
    vpi_printf("%s bits:", what);
    if (!it) vpi_printf(" NULL");
    while (it && (bit = vpi_scan(it))) vpi_printf(" %s", describe(bit));
    vpi_printf("\n");
}

static PLI_INT32 d47(PLI_BYTE8* data) {
    (void)data;
    vpiHandle top = vpi_handle_by_name("tb", NULL);
    vpiHandle it = vpi_iterate(vpiPrimitive, top), primitive;
    while (it && (primitive = vpi_scan(it)))
        vpi_printf("primitive %s prim_type=%d\n", describe(primitive),
                   (int)vpi_get(vpiPrimType, primitive));
    bits("scalar net a", vpi_handle_by_name("tb.a", NULL));
    bits("scalar reg rs", vpi_handle_by_name("tb.rs", NULL));
    bits("vector net nv", vpi_handle_by_name("tb.nv", NULL));
    bits("vector reg rv", vpi_handle_by_name("tb.rv", NULL));
    vpiHandle word = vpi_handle_by_name("tb.na[1]", NULL);
    vpi_printf("word %s\n", describe(word));
    bits("word na[1]", word);
    vpi_printf("by name %s\n", describe(vpi_handle_by_name("tb.rv[2]", NULL)));
    vpi_printf("by index %s\n", describe(vpi_handle_by_index(vpi_handle_by_name("tb.nv", NULL), 1)));
    it = vpi_iterate(vpiPort, vpi_handle_by_name("tb.k", NULL));
    vpiHandle port;
    while (it && (port = vpi_scan(it))) {
        vpi_printf("port %s low=%s\n", describe(port), describe(vpi_handle(vpiLowConn, port)));
        bits("  port", port);
    }
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task = {0};
    task.type = vpiSysTask;
    task.tfname = "$d47";
    task.calltf = d47;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
