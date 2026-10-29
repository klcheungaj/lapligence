/* Companion of A20-D12_delay_counts.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
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

static PLI_INT32 d12(PLI_BYTE8* data) {
    (void)data;
    (void)stage_argument();
    vpiHandle path = nth(vpiModPath, "tb.u", 0), gb = by_name("tb.gb");
    vpiHandle width = nth(vpiTchk, "tb.u", 0), nochange = nth(vpiTchk, "tb.u", 1);
    static const int counts[] = {1, 2, 3, 6, 12, 4};
    for (int i = 0; i < 6; ++i) {
        char label[32];
        snprintf(label, sizeof(label), "path %d", counts[i]);
        get(label, path, counts[i], 0, 0);
    }
    get("gate 1", gb, 1, 0, 0);
    get("gate 2", gb, 2, 0, 0);
    get("gate 3", gb, 3, 0, 0);
    get("width 1", width, 1, 0, 0);
    get("width 2", width, 2, 0, 0);
    get("nochange 2", nochange, 2, 0, 0);
    get("nochange 1", nochange, 1, 0, 0);
    static const double two[] = {5, 6}, four[] = {1, 1, 1, 1};
    put("put path 4", path, four, 4, 0, 0, 0);
    put("put path 2", path, two, 2, 0, 0, 0);
    get("path 12", path, 12, 0, 0);
    static const double gate[] = {7, 8};
    put("put gate 2", gb, gate, 2, 0, 0, 0);
    get("gate 3", gb, 3, 0, 0);
    put("put gate 1", gb, gate, 1, 0, 0, 0);
    put("put width 2", width, gate, 2, 0, 0, 0);
    put("put nochange 2", nochange, gate, 2, 0, 0, 0);
    get("nochange 2", nochange, 2, 0, 0);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d12";
    task.calltf = d12;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
