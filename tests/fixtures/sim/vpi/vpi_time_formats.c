/* R17: caller-selected representation, inherited units and raw global ticks. */
#include "vpi_user.h"
#include <math.h>
#include <string.h>

static int scaled(vpiHandle object, double expected) {
    s_vpi_time value = {0};
    value.type = vpiScaledRealTime;
    value.high = 0x11223344u;
    value.low = 0x55667788u;
    vpi_get_time(object, &value);
    return !vpi_chk_error(NULL) && value.type == vpiScaledRealTime &&
           fabs(value.real - expected) < 1e-9 &&
           value.high == 0x11223344u && value.low == 0x55667788u;
}

static PLI_INT32 probe(PLI_BYTE8* ignored) {
    (void)ignored;
    vpiHandle coarse = vpi_handle_by_name((PLI_BYTE8*)"tb.c", NULL);
    vpiHandle fine = vpi_handle_by_name((PLI_BYTE8*)"tb.f.value", NULL);
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    if (!coarse || !fine || !call || !scaled(coarse, 0.2) ||
        !scaled(fine, 200.0) || !scaled(call, 2.0) || !scaled(NULL, 2000.0))
        return 1;
    s_vpi_time value = {0};
    value.type = vpiSimTime;
    value.real = 81.25;
    vpi_get_time(coarse, &value);
    if (value.type != vpiSimTime || value.high != 0 || value.low != 2000 ||
        value.real != 81.25 || vpi_chk_error(NULL)) return 1;
    value.type = vpiSuppressTime;
    value.high = 19;
    value.low = 23;
    value.real = 29.5;
    vpi_get_time(fine, &value);
    if (value.type != vpiSuppressTime || value.high != 19 || value.low != 23 ||
        value.real != 29.5 || vpi_chk_error(NULL)) return 1;
    value.type = -123;
    vpi_get_time(fine, &value);
    s_vpi_error_info error = {0};
    if (!vpi_chk_error(&error) || !error.code ||
        strcmp((const char*)error.code, "LLG_VPI_TIME") != 0) return 1;
    vpi_printf("vpi time formats ok\n");
    return 0;
}

static void register_probe(void) {
    s_vpi_systf_data task = {0};
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$vpi_time_formats";
    task.calltf = probe;
    (void)vpi_register_systf(&task);
}
void (*vlog_startup_routines[])(void) = {register_probe, NULL};
