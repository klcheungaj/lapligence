/* Companion of A20-D22_string_delayed_put.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static void put(const char* name, const char* text, PLI_INT32 flags, PLI_UINT32 delay) {
    s_vpi_value value;
    value.format = vpiStringVal;
    value.value.str = (PLI_BYTE8*)text;
    s_vpi_time time;
    memset(&time, 0, sizeof(time));
    time.type = vpiSimTime;
    time.low = delay;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)name, NULL), &value, &time, flags);
    vpi_printf("put %s=%s mode=%d delay=%u level=%d\n", name, text, (int)flags, (unsigned)delay,
               (int)vpi_chk_error(NULL));
}

static PLI_INT32 d22(PLI_BYTE8* data) {
    (void)data;
    put("tb.s", "s3", vpiInertialDelay, 3);
    put("tb.s", "s4", vpiInertialDelay, 4);
    put("tb.t", "t2", vpiTransportDelay, 2);
    put("tb.t", "t5", vpiTransportDelay, 5);
    put("tb.t", "forced", vpiForceFlag, 0);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d22";
    task.calltf = d22;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
