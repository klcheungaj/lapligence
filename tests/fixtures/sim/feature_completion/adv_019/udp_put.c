/* ADV-019 vpi_put_value to sequential UDPs (IEEE 1800-2009 38.34). */
#include "sv_vpi_user.h"
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static PLI_INT32 now(void) {
    s_vpi_time time;
    time.type = vpiSimTime;
    vpi_get_time(NULL, &time);
    return (PLI_INT32)time.low;
}

static void put_as(const char* name, PLI_INT32 format, PLI_INT32 scalar, const char* text,
                   PLI_INT32 flags) {
    vpiHandle udp = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    s_vpi_value value;
    s_vpi_time time;
    memset(&time, 0, sizeof time);
    time.type = vpiSimTime;
    value.format = format;
    if (format == vpiScalarVal) value.value.scalar = scalar;
    else if (format == vpiIntVal) value.value.integer = scalar;
    else value.value.str = (PLI_BYTE8*)text;
    vpi_put_value(udp, &value, &time, flags);
    int error = level();
    value.format = vpiBinStrVal;
    vpi_get_value(udp, &value);
    vpi_printf("%d put %s level=%d value=%s\n", (int)now(), name, error,
               level() ? "?" : value.value.str);
}

static void put(const char* name, PLI_INT32 scalar) {
    put_as(name, vpiScalarVal, scalar, NULL, vpiNoDelay);
}

static PLI_INT32 clock_changed(p_cb_data data) {
    /* 60: during the clock's own rising edge, before the instances evaluate
     * it (d is 0). */
    if (now() == 60 && data->value->value.scalar == vpi1) {
        put("tb.un", vpi1);
        put("tb.ud", vpi1);
    }
    return 0;
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    put("tb.us", vpi1);
    return 0;
}

static PLI_INT32 udp_put(PLI_BYTE8* user) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = vpi_scan(args);
    s_vpi_value step;
    (void)user;
    vpi_free_object(args);
    step.format = vpiIntVal;
    vpi_get_value(arg, &step);
    switch (step.value.integer) {
    case 1: {
        s_cb_data cb;
        s_vpi_value value;
        s_vpi_time time;
        memset(&cb, 0, sizeof cb);
        time.type = vpiSuppressTime;
        value.format = vpiScalarVal;
        cb.reason = cbValueChange;
        cb.cb_rtn = clock_changed;
        cb.obj = vpi_handle_by_name((PLI_BYTE8*)"tb.clk", NULL);
        cb.time = &time;
        cb.value = &value;
        vpi_register_cb(&cb);
        put("tb.un", vpi1);
        put("tb.ua[0]", vpi1);
        put("tb.g[2].u", vpi1);
        put("tb.uw", vpi1);
        put("tb.ui", vpi1);
        break;
    }
    case 2:
        /* 21: ud, ua[1], g[0], g[1] and g[3] hold a pending transition to 1. */
        put("tb.ud", vpi0);
        put("tb.ua[1]", vpi0);
        put("tb.g[1].u", vpi0);
        put("tb.g[1].u", vpi1);
        put("tb.g[3].u", vpi0);
        break;
    case 3:
        put_as("tb.uw", vpiIntVal, 0, NULL, vpiNoDelay);
        put_as("tb.ua[1]", vpiBinStrVal, 0, "1", vpiNoDelay);
        put_as("tb.ua[0]", vpiScalarVal, vpiZ, NULL, vpiNoDelay);
        put_as("tb.ud", vpiScalarVal, vpi0, NULL, vpiForceFlag);
        break;
    case 4:
        put("tb.ud", vpi1);
        put("tb.uw", vpiX);
        break;
    default:
        put("tb.us", vpi1);
        put("tb.g[0].u", vpi1);
        break;
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data tf;
    s_cb_data cb;
    memset(&tf, 0, sizeof tf);
    tf.type = vpiSysTask;
    tf.tfname = (PLI_BYTE8*)"$udp_put";
    tf.calltf = udp_put;
    vpi_register_systf(&tf);
    memset(&cb, 0, sizeof cb);
    cb.reason = cbStartOfSimulation;
    cb.cb_rtn = start;
    vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
