/* SIM-043 A03: vpi_control(vpiStop/vpiFinish) from cbStartOfSimulation. The
 * request is chosen by SIM043_START_CONTROL (stop or finish). */
#include "vpi_user.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static const char* error_code(void) {
    static char code[64];
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    if (!vpi_chk_error(&info)) return "none";
    snprintf(code, sizeof(code), "%s", info.code ? (const char*)info.code : "?");
    return code;
}

static PLI_INT32 ended(p_cb_data data) {
    (void)data;
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("[%u] end\n", (unsigned)now.low);
    return 0;
}

static PLI_INT32 started(p_cb_data data) {
    (void)data;
    const char* mode = getenv("SIM043_START_CONTROL");
    if (mode && strcmp(mode, "finish") == 0) {
        PLI_INT32 status = vpi_control(vpiFinish, 7);
        vpi_printf("vpiFinish level 7 -> %d %s\n", (int)status, error_code());
    } else {
        PLI_INT32 status = vpi_control(vpiStop, 1);
        vpi_printf("vpiStop -> %d %s\n", (int)status, error_code());
    }
    vpi_printf("start returns\n");
    return 0;
}

static void startup(void) {
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbStartOfSimulation;
    cb.cb_rtn = started;
    vpi_register_cb(&cb);
    cb.reason = cbEndOfSimulation;
    cb.cb_rtn = ended;
    vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
