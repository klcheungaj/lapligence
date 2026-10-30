/* Companion of A20-D17_port_delays.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static void report(const char* label) {
    PLI_INT32 level = vpi_chk_error(NULL);
    if (level) vpi_printf("%s: error level=%d\n", label, (int)level);
    else vpi_printf("%s: ok\n", label);
}

static void get(const char* label, vpiHandle h, int count, int mtm, int pulse) {
    s_vpi_time da[27];
    s_vpi_delay delay;
    memset(da, 0, sizeof(da));
    memset(&delay, 0, sizeof(delay));
    delay.da = da;
    delay.no_of_delays = count;
    delay.time_type = vpiScaledRealTime;
    delay.mtm_flag = mtm;
    delay.pulsere_flag = pulse;
    vpi_get_delays(h, &delay);
    if (vpi_chk_error(NULL)) {
        report(label);
        return;
    }
    vpi_printf("%s:", label);
    for (int i = 0; i < count * (mtm ? 3 : 1) * (pulse ? 3 : 1); ++i) vpi_printf(" %g", da[i].real);
    vpi_printf("\n");
}

static void put(const char* label, vpiHandle h, const double* values, int count, int mtm,
                int pulse, int append) {
    s_vpi_time da[27];
    s_vpi_delay delay;
    memset(da, 0, sizeof(da));
    memset(&delay, 0, sizeof(delay));
    for (int i = 0; i < count * (mtm ? 3 : 1) * (pulse ? 3 : 1); ++i) {
        da[i].type = vpiScaledRealTime;
        da[i].real = values[i];
    }
    delay.da = da;
    delay.no_of_delays = count;
    delay.time_type = vpiScaledRealTime;
    delay.mtm_flag = mtm;
    delay.pulsere_flag = pulse;
    delay.append_flag = append;
    vpi_put_delays(h, &delay);
    report(label);
}

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

static PLI_INT32 d17(PLI_BYTE8* data) {
    (void)data;
    vpiHandle k = vpi_handle_by_name((PLI_BYTE8*)"tb.k", NULL);
    vpiHandle i = nth(vpiPort, k, 0), v = nth(vpiPort, k, 1), o = nth(vpiPort, k, 2);
    /* The port bits of v from its left (most significant) bit. */
    vpiHandle v1 = nth(vpiBit, v, 0), v0 = nth(vpiBit, v, 1);
    static const double rft[] = {5, 6, 7}, ones[] = {1, 1, 1}, bit0[] = {3, 4, 4},
                        bit1[] = {1, 2, 2}, mtm[] = {1, 2, 3, 4, 5, 6, 7, 8, 9},
                        pulse[] = {5, 5, 5, 6, 6, 6, 7, 7, 7};
    if (stage_argument() == 0) {
        get("i", i, 3, 0, 0);
        get("v bit", v0, 2, 0, 0);
        put("put i 5 6 7", i, rft, 3, 0, 0, 0);
        get("i", i, 3, 0, 0);
        get("i mtm", i, 3, 1, 0);
        put("put i mtm", i, mtm, 3, 1, 0, 0);
        get("i", i, 3, 0, 0);
        put("put i 5 6 7", i, rft, 3, 0, 0, 0);
        put("append i 1 1 1", i, ones, 3, 0, 0, 1);
        get("i", i, 3, 0, 0);
        put("put i 5 6 7", i, rft, 3, 0, 0, 0);
        put("put i pulsere", i, pulse, 3, 0, 1, 0);
        get("i pulsere", i, 3, 0, 1);
        put("put i 1 delay", i, ones, 1, 0, 0, 0);
        get("i", i, 3, 0, 0);
        get("output o", o, 3, 0, 0);
        put("put output o", o, ones, 3, 0, 0, 0);
        put("put v[0] 3 4 4", v0, bit0, 3, 0, 0, 0);
        put("put v[1] 1 2 2", v1, bit1, 3, 0, 0, 0);
        get("v[0]", v0, 3, 0, 0);
        get("v[1]", v1, 3, 0, 0);
        get("v", v, 3, 0, 0);
    } else {
        static const double two[] = {2, 3};
        put("put i 2 3", i, two, 2, 0, 0, 0);
        get("i", i, 3, 0, 0);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d17";
    task.calltf = d17;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
