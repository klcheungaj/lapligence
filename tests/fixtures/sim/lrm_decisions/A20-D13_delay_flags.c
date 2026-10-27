/* Companion of A20-D13_delay_flags.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static void report(const char* label) {
    s_vpi_error_info info;
    PLI_INT32 level = vpi_chk_error(&info);
    if (level) vpi_printf("%s: error level=%d\n", label, (int)level);
    else vpi_printf("%s: ok\n", label);
}

static void get(const char* label, vpiHandle h, int count, int mtm, int pulse) {
    s_vpi_time da[36];
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
    s_vpi_time da[36];
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

static vpiHandle by_name(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static vpiHandle nth(PLI_INT32 type, const char* scope, int index) {
    vpiHandle it = vpi_iterate(type, by_name(scope)), h = NULL;
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

static PLI_INT32 d13(PLI_BYTE8* data) {
    (void)data;
    vpiHandle path = nth(vpiModPath, "tb.u", 0), gb = by_name("tb.gb");
    if (stage_argument() == 0) {
        get("gate mtm", gb, 2, 1, 0);
        get("gate pulse", gb, 2, 0, 1);
        get("path pulse", path, 2, 0, 1);
        static const double eight[] = {8, 8};
        put("put path 8 8", path, eight, 2, 0, 0, 0);
        get("path pulse", path, 2, 0, 1);
        static const double plus[] = {1};
        put("append path 1", path, plus, 1, 0, 0, 1);
        get("path pulse", path, 2, 0, 1);
        static const double mtm[] = {1, 2, 3, 4, 5, 6};
        put("put gate mtm", gb, mtm, 2, 1, 0, 0);
        get("gate 3", gb, 3, 0, 0);
        static const double one[] = {1, 1};
        put("append gate 1 1", gb, one, 2, 0, 0, 1);
        get("gate 3", gb, 3, 0, 0);
        put("put gate pulse", gb, mtm, 2, 0, 1, 0);
        static const double negative[] = {-1, 2};
        put("put path negative", path, negative, 2, 0, 0, 0);
        get("path 2", path, 2, 0, 0);
    } else {
        static const double limits[] = {9, 9, 9};
        put("put path pulse", path, limits, 1, 0, 1, 0);
        get("path pulse", path, 2, 0, 1);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d13";
    task.calltf = d13;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
