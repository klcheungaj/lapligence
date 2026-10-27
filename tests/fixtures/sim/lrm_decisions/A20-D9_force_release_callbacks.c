/* Companion of A20-D9_force_release_callbacks.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 on_statement(p_cb_data data) {
    vpiHandle lhs = vpi_handle(vpiLhs, data->obj);
    vpi_printf("%s at %u: %s lhs=%s value=%s\n", (const char*)data->user_data,
               (unsigned)data->time->low, (const char*)vpi_get_str(vpiType, data->obj),
               lhs ? (const char*)vpi_get_str(vpiFullName, lhs) : "NULL",
               (const char*)data->value->value.str);
    return 0;
}

static void add(const char* label, PLI_INT32 reason, const char* name) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    static s_vpi_value value;
    value.format = vpiHexStrVal;
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = reason;
    cb.cb_rtn = on_statement;
    cb.obj = name ? vpi_handle_by_name((PLI_BYTE8*)name, NULL) : NULL;
    cb.time = &time;
    cb.value = &value;
    cb.user_data = (PLI_BYTE8*)label;
    vpiHandle handle = vpi_register_cb(&cb);
    vpi_printf("register %s: %s level=%d\n", label, handle ? "handle" : "NULL",
               (int)vpi_chk_error(NULL));
}

static PLI_INT32 d9(PLI_BYTE8* data) {
    (void)data;
    add("force", cbForce, NULL);
    add("release", cbRelease, NULL);
    add("n[7:4] force", cbForce, "tb.n[7:4]");
    add("v[0] force", cbForce, "tb.v[0]");
    add("assign", cbAssign, NULL);
    add("deassign", cbDeassign, "tb.v");
    return 0;
}

static PLI_INT32 d9_vpi(PLI_BYTE8* data) {
    (void)data;
    vpiHandle v = vpi_handle_by_name((PLI_BYTE8*)"tb.v", NULL);
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = 7;
    vpi_put_value(v, &value, NULL, vpiForceFlag);
    vpi_put_value(v, &value, NULL, vpiReleaseFlag);
    vpi_printf("vpi force and release done\n");
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d9";
    task.calltf = d9;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$d9_vpi";
    task.calltf = d9_vpi;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
