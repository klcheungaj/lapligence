/* ADV-020: vpi_get_delays/vpi_put_delays and cbTchkViolation on delays.sv
 * (IEEE 1800-2009 38.10, 38.32, 38.36.1). $delays(stage) reads and writes
 * the delays of a module path, a timing check, a primitive and a continuous
 * assignment in every format the record allows; the design's printed
 * transitions show each written delay. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int violations, hold_violations;

static vpiHandle by_name(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static vpiHandle first(PLI_INT32 type, const char* scope) {
    vpiHandle it = vpi_iterate(type, by_name(scope));
    vpiHandle h = it ? vpi_scan(it) : NULL;
    if (it) vpi_release_handle(it);
    return h;
}

/* The second timing check of tb.u: the $setuphold. */
static vpiHandle second_check(void) {
    vpiHandle it = vpi_iterate(vpiTchk, by_name("tb.u"));
    vpi_scan(it);
    vpiHandle h = vpi_scan(it);
    vpi_release_handle(it);
    return h;
}

static void report(const char* label) {
    s_vpi_error_info info;
    PLI_INT32 level = vpi_chk_error(&info);
    if (level) vpi_printf("%s: error level=%d %s\n", label, level, info.message);
    else vpi_printf("%s: ok\n", label);
}

/* Print `count` delays read with the given flags. */
static void get(const char* label, vpiHandle h, int count, int time_type, int mtm, int pulse) {
    s_vpi_time da[36];
    s_vpi_delay delay;
    memset(da, 0, sizeof(da));
    memset(&delay, 0, sizeof(delay));
    delay.da = da;
    delay.no_of_delays = count;
    delay.time_type = time_type;
    delay.mtm_flag = mtm;
    delay.pulsere_flag = pulse;
    vpi_get_delays(h, &delay);
    if (vpi_chk_error(NULL)) {
        report(label);
        return;
    }
    int values = count * (mtm ? 3 : 1) * (pulse ? 3 : 1);
    vpi_printf("%s:", label);
    for (int i = 0; i < values; ++i) {
        if (time_type == vpiSimTime) vpi_printf(" %u", (unsigned)da[i].low);
        else vpi_printf(" %g", da[i].real);
    }
    vpi_printf("\n");
}

static void put(const char* label, vpiHandle h, const double* values, int count, int mtm,
                int pulse, int append) {
    s_vpi_time da[36];
    s_vpi_delay delay;
    memset(da, 0, sizeof(da));
    memset(&delay, 0, sizeof(delay));
    int n = count * (mtm ? 3 : 1) * (pulse ? 3 : 1);
    for (int i = 0; i < n; ++i) {
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

static PLI_INT32 on_violation(p_cb_data data) {
    vpi_printf("violation at %u: %s line %d\n", (unsigned)data->time->low,
               vpi_get(vpiTchkType, data->obj) == vpiSetup ? "$setup" : "other",
               vpi_get(vpiLineNo, data->obj));
    ++violations;
    return 0;
}

static PLI_INT32 on_hold(p_cb_data data) {
    (void)data;
    ++hold_violations;
    return 0;
}

static void watch(vpiHandle obj, PLI_INT32 (*routine)(p_cb_data)) {
    s_cb_data cb;
    s_vpi_time time;
    memset(&cb, 0, sizeof(cb));
    time.type = vpiSimTime;
    cb.reason = cbTchkViolation;
    cb.cb_rtn = routine;
    cb.obj = obj;
    cb.time = &time;
    vpiHandle handle = vpi_register_cb(&cb);
    vpi_printf("cbTchkViolation %s: %s\n", obj ? "one check" : "all checks",
               handle ? "registered" : "refused");
}

static void stage0(void) {
    vpiHandle path = first(vpiModPath, "tb.u"), setup = first(vpiTchk, "tb.u");
    vpiHandle setuphold = second_check();
    vpiHandle gb = by_name("tb.gb"), nb = by_name("tb.nb"), cont = first(vpiContAssign, "tb");
    get("path 2", path, 2, vpiScaledRealTime, 0, 0);
    get("path 1", path, 1, vpiScaledRealTime, 0, 0);
    get("path 3", path, 3, vpiScaledRealTime, 0, 0);
    get("path 6", path, 6, vpiScaledRealTime, 0, 0);
    get("path 12 ps", path, 12, vpiSimTime, 0, 0);
    get("path pulse", path, 2, vpiScaledRealTime, 0, 1);
    get("path mtm pulse", path, 1, vpiScaledRealTime, 1, 1);
    get("path 4", path, 4, vpiScaledRealTime, 0, 0);
    get("gb 3", gb, 3, vpiScaledRealTime, 0, 0);
    get("gb pulse", gb, 2, vpiScaledRealTime, 0, 1);
    get("gb 1", gb, 1, vpiScaledRealTime, 0, 0);
    get("nb 2", nb, 2, vpiScaledRealTime, 0, 0);
    get("cont 3", cont, 3, vpiScaledRealTime, 0, 0);
    get("setup 1", setup, 1, vpiScaledRealTime, 0, 0);
    get("setup 2", setup, 2, vpiScaledRealTime, 0, 0);
    get("setup pulse", setup, 1, vpiScaledRealTime, 0, 1);
    get("setuphold 2", setuphold, 2, vpiScaledRealTime, 0, 0);
    get("net", by_name("tb.y"), 2, vpiScaledRealTime, 0, 0);
    get("suppress", path, 2, vpiSuppressTime, 0, 0);
    vpi_get_delays(path, NULL);
    report("null record");
    watch(NULL, on_violation);
    watch(setuphold, on_hold);
    watch(path, on_hold);
    static const double path_delays[] = {7, 8};
    put("put path 7 8", path, path_delays, 2, 0, 0, 0);
    get("path 2", path, 2, vpiScaledRealTime, 0, 0);
    get("path pulse", path, 2, vpiScaledRealTime, 0, 1);
    /* min:typ:max values: the model uses typ. */
    static const double gate_delays[] = {1, 5, 9, 2, 6, 10};
    put("put gb mtm", gb, gate_delays, 2, 1, 0, 0);
    get("gb 3", gb, 3, vpiScaledRealTime, 0, 0);
    get("gb mtm", gb, 2, vpiScaledRealTime, 1, 0);
    put("put nb", nb, path_delays, 2, 0, 0, 0);
    put("put cont", cont, path_delays, 2, 0, 0, 0);
    static const double negative[] = {-1, 2};
    put("put path negative", path, negative, 2, 0, 0, 0);
    put("put gb pulse", gb, gate_delays, 2, 0, 1, 0);
}

static PLI_INT32 delays(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &value);
    vpi_release_handle(args);
    int stage = (int)value.value.integer;
    vpiHandle path = first(vpiModPath, "tb.u"), setup = first(vpiTchk, "tb.u");
    vpi_printf("-- stage %d\n", stage);
    switch (stage) {
        case 0:
            stage0();
            break;
        case 1: {
            static const double one[] = {1};
            put("append path 1", path, one, 1, 0, 0, 1);
            get("path 2", path, 2, vpiScaledRealTime, 0, 0);
            get("path pulse", path, 2, vpiScaledRealTime, 0, 1);
            break;
        }
        case 2: {
            static const double limits[] = {8, 8, 8, 9, 9, 9};
            put("put path pulse", path, limits, 2, 0, 1, 0);
            get("path pulse", path, 2, vpiScaledRealTime, 0, 1);
            break;
        }
        case 3: {
            vpiHandle setuphold = second_check();
            get("setup 1", setup, 1, vpiSimTime, 0, 0);
            static const double one[] = {1};
            put("put setup 1", setup, one, 1, 0, 0, 0);
            get("setup 1", setup, 1, vpiScaledRealTime, 0, 0);
            static const double pair[] = {5, 2};
            put("put setuphold 5 2", setuphold, pair, 2, 0, 0, 0);
            get("setuphold 2", setuphold, 2, vpiScaledRealTime, 0, 0);
            put("put setuphold 1", setuphold, pair, 1, 0, 0, 0);
            break;
        }
        case 4: {
            static const double two[] = {2};
            put("append setup 2", setup, two, 1, 0, 0, 1);
            get("setup 1", setup, 1, vpiScaledRealTime, 0, 0);
            break;
        }
        default: {
            vpi_printf("violations=%d one-check=%d\n", violations, hold_violations);
            /* A limit that may not be negative stops at 0. */
            static const double negative[] = {-5};
            put("append setup -5", setup, negative, 1, 0, 0, 1);
            get("setup 1", setup, 1, vpiScaledRealTime, 0, 0);
            break;
        }
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$delays";
    task.calltf = delays;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
