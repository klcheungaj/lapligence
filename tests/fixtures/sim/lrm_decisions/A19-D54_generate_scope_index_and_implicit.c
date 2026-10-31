/* Decision A19-D54: the index and implicit declaration of generate scopes.
 * Companion of A19-D54_generate_scope_index_and_implicit.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>
/* Each generate scope below `scope`, with its vpiImplicitDecl and its
 * vpiIndex (NULL outside a generate scope array), then its own scopes. */
static void walk(vpiHandle scope, int depth) {
    vpiHandle it = vpi_iterate(vpiInternalScope, scope), s;
    while (it && (s = vpi_scan(it))) {
        if (vpi_get(vpiType, s) != vpiGenScope) continue;
        s_vpi_error_info error;
        int implicit = (int)vpi_get(vpiImplicitDecl, s);
        int failed = vpi_chk_error(&error);
        vpiHandle index = vpi_handle(vpiIndex, s);
        int index_failed = vpi_chk_error(&error);
        char text[40] = "NULL";
        if (index) {
            s_vpi_value value;
            memset(&value, 0, sizeof value);
            value.format = vpiIntVal;
            vpi_get_value(index, &value);
            snprintf(text, sizeof text, "%s=%d", (const char*)vpi_get_str(vpiType, index), (int)value.value.integer);
        }
        vpi_printf("%*s%s implicit=%d%s index=%s%s\n", depth * 2, "", (const char*)vpi_get_str(vpiFullName, s), implicit, failed ? "(error)" : "", text, index_failed ? "(error)" : "");
        walk(s, depth + 1);
    }
}
static PLI_INT32 d54(PLI_BYTE8* data) {
    (void)data;
    walk(vpi_handle_by_name("tb", NULL), 0);
    return 0;
}
static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d54";
    task.calltf = d54;
    vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {startup, 0};
