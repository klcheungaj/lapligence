/* Companion of A20-D19_runtime_delays.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static void report(const char* label) {
    PLI_INT32 level = vpi_chk_error(NULL);
    if (level) vpi_printf("%s: error level=%d\n", label, (int)level);
    else vpi_printf("%s: ok\n", label);
}

static void get(const char* label, vpiHandle h, int count) {
    s_vpi_time da[3];
    s_vpi_delay delay;
    memset(da, 0, sizeof(da));
    memset(&delay, 0, sizeof(delay));
    delay.da = da;
    delay.no_of_delays = count;
    delay.time_type = vpiScaledRealTime;
    vpi_get_delays(h, &delay);
    if (vpi_chk_error(NULL)) {
        report(label);
        return;
    }
    vpi_printf("%s:", label);
    for (int i = 0; i < count; ++i) vpi_printf(" %g", da[i].real);
    vpi_printf("\n");
}

static void put(const char* label, vpiHandle h, double rise, double fall) {
    s_vpi_time da[2];
    s_vpi_delay delay;
    memset(da, 0, sizeof(da));
    memset(&delay, 0, sizeof(delay));
    da[0].type = da[1].type = vpiScaledRealTime;
    da[0].real = rise;
    da[1].real = fall;
    delay.da = da;
    delay.no_of_delays = 2;
    delay.time_type = vpiScaledRealTime;
    vpi_put_delays(h, &delay);
    report(label);
}

static vpiHandle by_name(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static vpiHandle nth(PLI_INT32 type, vpiHandle scope, int index) {
    vpiHandle it = vpi_iterate(type, scope), h = NULL;
    for (int i = 0; it && i <= index; ++i) h = vpi_scan(it);
    if (h && it) vpi_release_handle(it);
    return h;
}

static int stage_argument(void) {
    vpiHandle args = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &value);
    vpi_release_handle(args);
    return (int)value.value.integer;
}

static PLI_INT32 d19(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = by_name("tb"), gate = by_name("tb.gr");
    int stage = stage_argument();
    if (stage == 0) {
        get("gate", gate, 2);
        put("put gate 7 8", gate, 7, 8);
        get("gate", gate, 2);
        vpiHandle cont = nth(vpiContAssign, tb, 0);
        get("assign #3", cont, 3);
        put("put assign", cont, 1, 1);
        get("net #4", by_name("tb.wn"), 3);
        put("put net", by_name("tb.wn"), 1, 1);
        vpiHandle initial = nth(vpiProcess, tb, 1);
        vpiHandle control = nth(vpiStmt, vpi_handle(vpiStmt, initial), 0);
        get("delay control #1", control, 1);
    } else {
        get("gate", gate, 2);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d19";
    task.calltf = d19;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
