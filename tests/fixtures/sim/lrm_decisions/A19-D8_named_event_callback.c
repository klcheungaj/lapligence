/* Companion of A19-D8_named_event_callback.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static PLI_INT32 on_event(p_cb_data data) {
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("cb %s at %d value=%s\n", vpi_get_str(vpiFullName, data->obj), (int)now.low,
               data->value ? "set" : "NULL");
    return 0;
}

static vpiHandle event_handle(void) { return vpi_handle_by_name("tb.ev", NULL); }

static PLI_INT32 watch(PLI_BYTE8* data) {
    (void)data;
    vpiHandle ev = event_handle();
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(ev, &value);
    vpi_printf("%s get level=%d\n", vpi_get_str(vpiType, ev), (int)vpi_chk_error(NULL));
    s_cb_data cb;
    s_vpi_time time;
    s_vpi_value format;
    memset(&cb, 0, sizeof(cb));
    time.type = vpiSimTime;
    format.format = vpiIntVal;
    cb.reason = cbValueChange;
    cb.cb_rtn = on_event;
    cb.obj = ev;
    cb.time = &time;
    cb.value = &format;
    vpi_printf("watch %s\n", vpi_register_cb(&cb) ? "registered" : "refused");
    return 0;
}

static PLI_INT32 put(PLI_BYTE8* data) {
    (void)data;
    vpi_put_value(event_handle(), NULL, NULL, vpiNoDelay);
    vpi_printf("put level=%d\n", (int)vpi_chk_error(NULL));
    return 0;
}

static PLI_INT32 delayed(PLI_BYTE8* data) {
    (void)data;
    s_vpi_time delay;
    delay.type = vpiSimTime;
    delay.high = 0;
    delay.low = 1;
    vpi_put_value(event_handle(), NULL, &delay, vpiInertialDelay);
    vpi_printf("delayed put level=%d\n", (int)vpi_chk_error(NULL));
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d8_watch";
    task.calltf = watch;
    vpi_register_systf(&task);
    task.tfname = "$d8_put";
    task.calltf = put;
    vpi_register_systf(&task);
    task.tfname = "$d8_delay";
    task.calltf = delayed;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
