/* Decision A19-D78: automatics while their frame ends (see the .sv). Each
 * step places cbEndOfFrame on the active frame, tries a delayed put and a
 * value change callback on its last automatic, then writes it with
 * vpiNoDelay. The callback prints every automatic of the ending frame. */
#include "sv_vpi_user.h"
#include <string.h>

static int failed(void) { return vpi_chk_error(NULL) != 0; }

static void print_automatics(vpiHandle frame) {
    vpiHandle it = vpi_iterate(vpiAutomatics, frame), x;
    while (it && (x = vpi_scan(it))) {
        s_vpi_value value;
        memset(&value, 0, sizeof value);
        value.format = vpiIntVal;
        vpi_get_value(x, &value);
        int error = failed();
        vpi_printf(" %s=%d", vpi_get_str(vpiName, x), error ? -1 : (int)value.value.integer);
        if (error) vpi_printf(" (error)");
    }
    vpi_printf("\n");
}

static PLI_INT32 ended(p_cb_data data) {
    vpi_printf("  cbEndOfFrame %s:", vpi_get_str(vpiFullName, data->obj));
    print_automatics(data->obj);
    return 0;
}

static PLI_INT32 changed(p_cb_data data) {
    (void)data;
    vpi_printf("  value change\n");
    return 0;
}

static PLI_INT32 calltf(PLI_BYTE8* user_data) {
    (void)user_data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = vpi_scan(args);
    vpi_free_object(args);
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiIntVal;
    vpi_get_value(arg, &value);
    vpi_printf("step %d\n", (int)value.value.integer);
    vpiHandle frame = vpi_handle(vpiFrame, NULL);
    s_cb_data cb;
    memset(&cb, 0, sizeof cb);
    cb.reason = cbEndOfFrame;
    cb.cb_rtn = ended;
    cb.obj = frame;
    vpiHandle placed = vpi_register_cb(&cb);
    vpi_printf("  place cbEndOfFrame: %s (error %d)\n", placed ? "ok" : "NULL", failed());
    vpiHandle it = vpi_iterate(vpiAutomatics, frame), x, last = NULL;
    while (it && (x = vpi_scan(it))) last = x;
    s_vpi_value write;
    memset(&write, 0, sizeof write);
    write.format = vpiIntVal;
    write.value.integer = 77;
    s_vpi_time delay;
    memset(&delay, 0, sizeof delay);
    delay.type = vpiSimTime;
    delay.low = 1;
    vpiHandle event = vpi_put_value(last, &write, &delay, vpiInertialDelay);
    vpi_printf("  delayed put to %s: %s (error %d)\n", vpi_get_str(vpiName, last),
               event ? "event" : "NULL", failed());
    s_cb_data watch;
    s_vpi_time watch_time;
    s_vpi_value watch_value;
    memset(&watch, 0, sizeof watch);
    memset(&watch_time, 0, sizeof watch_time);
    memset(&watch_value, 0, sizeof watch_value);
    watch_time.type = vpiSuppressTime;
    watch_value.format = vpiSuppressVal;
    watch.reason = cbValueChange;
    watch.cb_rtn = changed;
    watch.obj = last;
    watch.time = &watch_time;
    watch.value = &watch_value;
    vpiHandle watching = vpi_register_cb(&watch);
    vpi_printf("  value change callback: %s (error %d)\n", watching ? "ok" : "NULL", failed());
    write.value.integer = 78;
    vpi_put_value(last, &write, NULL, vpiNoDelay);
    vpi_printf("  put %s=78 (error %d):", vpi_get_str(vpiName, last), failed());
    print_automatics(frame);
    return 0;
}

static void register_d78(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = "$d78";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {register_d78, 0};
