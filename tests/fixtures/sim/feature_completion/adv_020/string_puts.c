/* ADV-020: delayed vpi_put_value to string variables (IEEE 1800-2009
 * 38.34). Companion of string_puts.sv. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle pending;

static void report(const char* label) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("%s: level=%d code=%s\n", label, (int)level,
               level && info.code ? (const char*)info.code : "-");
}

static vpiHandle put(const char* name, const char* text, PLI_INT32 flags, PLI_UINT32 delay) {
    s_vpi_value value;
    value.format = vpiStringVal;
    value.value.str = (PLI_BYTE8*)text;
    s_vpi_time time = {vpiSimTime, 0, delay, 0.0};
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpiHandle event = vpi_put_value(handle, &value, &time, flags);
    char label[96];
    snprintf(label, sizeof(label), "put %s=%s flags=%d delay=%u %s", name, text, (int)flags,
             (unsigned)delay, event ? "event" : "no event");
    report(label);
    return event;
}

static PLI_INT32 sp_put(PLI_BYTE8* data) {
    (void)data;
    /* Inertial: the second put replaces the first (both pending). */
    put("tb.s", "s3", vpiInertialDelay, 3);
    put("tb.s", "s4", vpiInertialDelay, 4);
    /* Transport: both happen, at 2 and 5. */
    put("tb.t", "t2", vpiTransportDelay, 2);
    put("tb.t", "t5", vpiTransportDelay, 5);
    /* Kept to be cancelled by $sp_cancel. */
    pending = put("tb.u", "u6", vpiInertialDelay | vpiReturnEvent, 6);
    /* Without a time the put is refused. */
    s_vpi_value value;
    value.format = vpiStringVal;
    value.value.str = (PLI_BYTE8*)"x";
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.s", NULL), &value, NULL, vpiInertialDelay);
    report("put tb.s without a time");
    /* Still refused: force on a string, delays on elements and properties. */
    put("tb.s", "forced", vpiForceFlag, 0);
    vpiHandle q = vpi_handle_by_name((PLI_BYTE8*)"tb.q", NULL);
    vpiHandle element = vpi_handle_by_index(q, 0);
    s_vpi_time time = {vpiSimTime, 0, 2, 0.0};
    value.value.str = (PLI_BYTE8*)"q2";
    vpi_put_value(element, &value, &time, vpiInertialDelay);
    report("put tb.q[0]=q2 delay=2");
    return 0;
}

static PLI_INT32 sp_cancel(PLI_BYTE8* data) {
    (void)data;
    vpi_put_value(pending, NULL, NULL, vpiCancelEvent);
    report("cancel u6");
    return 0;
}

static void registration(void) {
    static const char* names[] = {"$sp_put", "$sp_cancel"};
    PLI_INT32 (*routines[])(PLI_BYTE8*) = {sp_put, sp_cancel};
    for (int i = 0; i < 2; ++i) {
        s_vpi_systf_data task;
        memset(&task, 0, sizeof(task));
        task.type = vpiSysTask;
        task.tfname = (PLI_BYTE8*)names[i];
        task.calltf = routines[i];
        vpi_register_systf(&task);
    }
}

void (*vlog_startup_routines[])(void) = {registration, 0};
