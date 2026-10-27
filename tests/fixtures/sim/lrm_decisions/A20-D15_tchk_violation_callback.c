/* Companion of A20-D15_tchk_violation_callback.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 on_violation(p_cb_data data) {
    s_vpi_value value;
    value.format = vpiBinStrVal;
    vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)"tb.u.notifier", NULL), &value);
    vpi_printf("%s: %s type=%d at %u notifier=%s value=%s\n", (const char*)data->user_data,
               (const char*)vpi_get_str(vpiType, data->obj), (int)vpi_get(vpiTchkType, data->obj),
               (unsigned)data->time->low, value.value.str, data->value ? "set" : "NULL");
    return 0;
}

static void add(const char* label, vpiHandle obj) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbTchkViolation;
    cb.cb_rtn = on_violation;
    cb.obj = obj;
    cb.time = &time;
    cb.user_data = (PLI_BYTE8*)label;
    vpiHandle handle = vpi_register_cb(&cb);
    vpi_printf("register %s: %s level=%d\n", label, handle ? "handle" : "NULL",
               (int)vpi_chk_error(NULL));
}

static PLI_INT32 d15(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiTchk, vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL));
    vpi_scan(it);
    vpiHandle hold = vpi_scan(it);
    vpi_release_handle(it);
    add("all", NULL);
    add("hold", hold);
    add("module", vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d15";
    task.calltf = d15;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
