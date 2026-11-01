/* Companion of A19-D74_sequential_udp_put.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static void put(const char* path, PLI_INT32 bit, PLI_INT32 flags) {
    vpiHandle udp = vpi_handle_by_name((PLI_BYTE8*)path, NULL);
    s_vpi_value value;
    s_vpi_time time;
    memset(&time, 0, sizeof time);
    time.type = vpiSimTime;
    time.low = 3;
    value.format = vpiScalarVal;
    value.value.scalar = bit;
    vpi_put_value(udp, &value, &time, flags);
    vpi_printf("put %s %d flags=%d error=%d\n", path, (int)bit, (int)flags, level() != 0);
}

static PLI_INT32 d74(PLI_BYTE8* user) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = vpi_scan(args);
    s_vpi_value step;
    (void)user;
    vpi_free_object(args);
    step.format = vpiIntVal;
    vpi_get_value(arg, &step);
    switch (step.value.integer) {
        case 1:
            put("tb.u1", vpi0, vpiNoDelay);
            put("tb.u2", vpi0, vpiNoDelay);
            put("tb.u3", vpi1, vpiNoDelay);
            break;
        case 2:
            put("tb.u1", vpi0, vpiInertialDelay);
            put("tb.u2", vpiX, vpiNoDelay);
            put("tb.u3", vpi0, vpiForceFlag);
            put("tb.u4", vpi1, vpiNoDelay);
            break;
        default:
            put("tb.u2", vpi1, vpiNoDelay);
            put("tb.u1", vpiZ, vpiNoDelay);
            break;
    }
    return 0;
}

static void register_d74(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof tf);
    tf.type = vpiSysTask;
    tf.tfname = (PLI_BYTE8*)"$d74";
    tf.calltf = d74;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {register_d74, 0};
