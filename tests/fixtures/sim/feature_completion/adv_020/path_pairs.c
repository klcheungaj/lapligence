/* ADV-020: the delays of every input and output pair of the multiple path
 * declarations of path_pairs.sv (IEEE 1800-2009 30.4.6, 37.35, 38.10,
 * 38.32). $pairs(stage) prints the paths of tb.u and tb.v, then writes some
 * pairs of tb.u. */
#include "sv_vpi_user.h"
#include <string.h>

static vpiHandle scope(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static const char* name_of(vpiHandle h) {
    const char* name = h ? vpi_get_str(vpiFullName, h) : NULL;
    return name ? name : "NULL";
}

/* Rise and fall delays; with `pulse`, each with its reject and error limit. */
static void access_delays(vpiHandle path, double* values, int pulse, int append, int put) {
    s_vpi_time da[6];
    s_vpi_delay delay;
    int count = pulse ? 6 : 2;
    memset(da, 0, sizeof(da));
    memset(&delay, 0, sizeof(delay));
    delay.da = da;
    delay.no_of_delays = 2;
    delay.time_type = vpiScaledRealTime;
    delay.pulsere_flag = pulse;
    delay.append_flag = append;
    for (int i = 0; put && i < count; ++i) {
        da[i].type = vpiScaledRealTime;
        da[i].real = values[i];
    }
    if (put) vpi_put_delays(path, &delay);
    else vpi_get_delays(path, &delay);
    s_vpi_error_info info;
    if (vpi_chk_error(&info)) vpi_printf("delays: error %s\n", info.message);
    for (int i = 0; !put && i < count; ++i) values[i] = da[i].real;
}

static void print(const char* module) {
    vpiHandle it = vpi_iterate(vpiModPath, scope(module)), path;
    vpi_printf("%s:", module);
    while (it && (path = vpi_scan(it))) {
        double values[6];
        access_delays(path, values, 0, 0, 0);
        vpiHandle condition = vpi_handle(vpiCondition, path);
        vpi_printf(" %s%s%s=%g,%g", name_of(path) + strlen(module) + 1, condition ? "?" : "",
                   vpi_get(vpiModPathHasIfNone, path) ? "!" : "", values[0], values[1]);
    }
    vpi_printf("\n");
}

/* The `nth` path of tb.u named `tb.u.<name>`, from 0. */
static vpiHandle find(const char* name, int nth) {
    vpiHandle it = vpi_iterate(vpiModPath, scope("tb.u")), path;
    while (it && (path = vpi_scan(it)))
        if (strcmp(name_of(path) + strlen("tb.u."), name) == 0 && nth-- == 0) {
            vpi_release_handle(it);
            return path;
        }
    vpi_printf("no path %s\n", name);
    return NULL;
}

static void write(const char* name, int nth, double rise, double fall, int append) {
    double values[2] = {rise, fall};
    access_delays(find(name, nth), values, 0, append, 1);
}

static PLI_INT32 pairs(PLI_BYTE8* data) {
    (void)data;
    vpiHandle args = vpi_iterate(vpiArgument, vpi_handle(vpiSysTfCall, NULL));
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(vpi_scan(args), &value);
    vpi_release_handle(args);
    if (value.value.integer == 1) {
        write("a$y", 0, 1, 2, 0);
        write("b$z", 0, 3, 4, 0);
        write("c$z", 0, 5, 6, 1);
        write("d$w", 0, 7, 8, 0);
        write("e$w", 1, 9, 10, 0);
    } else if (value.value.integer == 2) {
        double values[6] = {10, 3, 6, 10, 3, 6};
        access_delays(find("c$y", 0), values, 1, 0, 1);
        memset(values, 0, sizeof(values));
        access_delays(find("c$y", 0), values, 1, 0, 0);
        vpi_printf("c$y: %g %g %g %g %g %g\n", values[0], values[1], values[2], values[3],
                   values[4], values[5]);
        access_delays(find("b$y", 0), values, 1, 0, 0);
        vpi_printf("b$y: %g %g %g %g %g %g\n", values[0], values[1], values[2], values[3],
                   values[4], values[5]);
    }
    print("tb.u");
    print("tb.v");
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$pairs";
    task.calltf = pairs;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
