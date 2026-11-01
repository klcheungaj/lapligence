/* Companion of A20-D21_strength_values.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 changed(p_cb_data data) {
    if (data->value->format == vpiStrengthVal)
        vpi_printf("  %s at %d: logic=%d s0=%d s1=%d\n", (const char*)data->user_data,
                   (int)data->time->low, (int)data->value->value.strength[0].logic,
                   (int)data->value->value.strength[0].s0, (int)data->value->value.strength[0].s1);
    else
        vpi_printf("  %s at %d: %s\n", (const char*)data->user_data, (int)data->time->low,
                   data->value->value.str);
    return 0;
}

static void watch(PLI_INT32 format, const char* label) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    static s_vpi_value values[2];
    static int used;
    s_vpi_value* value = &values[used++ % 2];
    value->format = format;
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbValueChange;
    cb.cb_rtn = changed;
    cb.obj = vpi_handle_by_name((PLI_BYTE8*)"tb.w", NULL);
    cb.time = &time;
    cb.value = value;
    cb.user_data = (PLI_BYTE8*)label;
    vpi_register_cb(&cb);
}

static PLI_INT32 d21(PLI_BYTE8* data) {
    (void)data;
    static const char* names[] = {"tb.w1", "tb.wl", "tb.wh", "tb.wx", "tb.wz", "tb.d"};
    for (int i = 0; i < 6; ++i) {
        s_vpi_value value;
        value.format = vpiStrengthVal;
        vpi_get_value(vpi_handle_by_name((PLI_BYTE8*)names[i], NULL), &value);
        vpi_printf("%s: logic=%d s0=%d s1=%d\n", names[i], (int)value.value.strength[0].logic,
                   (int)value.value.strength[0].s0, (int)value.value.strength[0].s1);
    }
    watch(vpiStrengthVal, "strength");
    watch(vpiBinStrVal, "value");
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d21";
    task.calltf = d21;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
