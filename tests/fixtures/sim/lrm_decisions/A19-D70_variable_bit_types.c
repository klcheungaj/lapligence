/* Companion of A19-D70_variable_bit_types.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static void show(const char* how, vpiHandle bit) {
    s_vpi_value value;
    if (!bit) {
        vpi_printf("  %s: NULL\n", how);
        return;
    }
    value.format = vpiBinStrVal;
    vpi_get_value(bit, &value);
    vpi_printf("  %s: %s type=%d %s value=%s\n", how, vpi_get_str(vpiFullName, bit),
               (int)vpi_get(vpiType, bit), vpi_get_str(vpiType, bit), value.value.str);
}

static void bits(const char* name, const char* bit_name) {
    vpiHandle object = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpiHandle it, first;
    vpi_printf("%s:\n", name);
    show("by index 1", vpi_handle_by_index(object, 1));
    it = vpi_iterate(vpiBit, object);
    first = it ? vpi_scan(it) : NULL;
    if (first) vpi_release_handle(it);
    show("first of vpiBit", first);
    show("by name", vpi_handle_by_name((PLI_BYTE8*)bit_name, NULL));
}

static PLI_INT32 d70(PLI_BYTE8* user) {
    vpiHandle m = vpi_handle_by_name((PLI_BYTE8*)"tb.m", NULL);
    vpiHandle n = vpi_handle_by_name((PLI_BYTE8*)"tb.n", NULL);
    (void)user;
    bits("tb.v", "tb.v[2]");
    bits("tb.b", "tb.b[2]");
    bits("tb.i", "tb.i[2]");
    bits("tb.g", "tb.g[2]");
    bits("tb.w", "tb.w[2]");
    vpi_printf("tb.m[0]:\n");
    show("by index 1", vpi_handle_by_index(vpi_handle_by_index(m, 0), 1));
    show("by name", vpi_handle_by_name((PLI_BYTE8*)"tb.m[0][2]", NULL));
    vpi_printf("tb.n[0]:\n");
    show("by index 1", vpi_handle_by_index(vpi_handle_by_index(n, 0), 1));
    return 0;
}

static void register_d70(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof tf);
    tf.type = vpiSysTask;
    tf.tfname = (PLI_BYTE8*)"$d70";
    tf.calltf = d70;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {register_d70, 0};
