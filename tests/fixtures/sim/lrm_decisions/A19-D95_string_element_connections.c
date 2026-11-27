#include "sv_vpi_user.h"
#include <string.h>

static int failed(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) != 0;
}

static void show(const char* label, vpiHandle h) {
    if (!h) {
        vpi_printf(" %s=NULL", label);
        return;
    }
    const char* name = vpi_get_str(vpiFullName, h);
    (void)failed();
    vpi_printf(" %s=%d:%s", label, (int)vpi_get(vpiType, h), name ? name : "NULL");
    s_vpi_value v;
    v.format = vpiStringVal;
    vpi_get_value(h, &v);
    if (failed()) vpi_printf("(no value)");
    else vpi_printf("[%s]", v.value.str);
    /* An element's vpiParent is its array; a module variable's is not tested. */
    if (strcmp(label, "high")) return;
    vpiHandle parent = vpi_handle(vpiParent, h);
    (void)failed();
    if (parent) vpi_printf(" parent=%d", (int)vpi_get(vpiType, parent));
}

static PLI_INT32 pp(PLI_BYTE8* data) {
    (void)data;
    vpiHandle module = vpi_handle_by_name((PLI_BYTE8*)"tb.c", NULL);
    vpiHandle it = vpi_iterate(vpiPort, module), port;
    while (it && (port = vpi_scan(it)) != NULL) {
        vpi_printf("port %s dir=%d", vpi_get_str(vpiName, port), (int)vpi_get(vpiDirection, port));
        show("high", vpi_handle(vpiHighConn, port));
        (void)failed();
        show("low", vpi_handle(vpiLowConn, port));
        (void)failed();
        vpi_printf("\n");
    }
    return 0;
}

static PLI_INT32 ps(PLI_BYTE8* data) {
    (void)data;
    vpiHandle module = vpi_handle_by_name((PLI_BYTE8*)"tb.c", NULL);
    vpiHandle it = vpi_iterate(vpiPort, module), port;
    while (it && (port = vpi_scan(it)) != NULL) {
        if (strcmp(vpi_get_str(vpiName, port), "sq")) continue;
        s_vpi_value v;
        v.format = vpiStringVal;
        v.value.str = (PLI_BYTE8*)"vpi";
        vpi_put_value(vpi_handle(vpiLowConn, port), &v, NULL, vpiNoDelay);
        vpi_printf("put sq%s\n", failed() ? " (error)" : "");
        vpi_free_object(it);
        break;
    }
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$pp";
    task.calltf = pp;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$ps";
    task.calltf = ps;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, NULL};
