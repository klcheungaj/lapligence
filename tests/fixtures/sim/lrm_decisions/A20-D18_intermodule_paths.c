/* Companion of A20-D18_intermodule_paths.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static void report(const char* label) {
    PLI_INT32 level = vpi_chk_error(NULL);
    if (level) vpi_printf("%s: error level=%d\n", label, (int)level);
    else vpi_printf("%s: ok\n", label);
}

static void get(const char* label, vpiHandle h, int count) {
    s_vpi_time da[12];
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

static void put(const char* label, vpiHandle h, const double* values, int count) {
    s_vpi_time da[3];
    s_vpi_delay delay;
    memset(da, 0, sizeof(da));
    memset(&delay, 0, sizeof(delay));
    for (int i = 0; i < count; ++i) {
        da[i].type = vpiScaledRealTime;
        da[i].real = values[i];
    }
    delay.da = da;
    delay.no_of_delays = count;
    delay.time_type = vpiScaledRealTime;
    vpi_put_delays(h, &delay);
    report(label);
}

static vpiHandle nth(PLI_INT32 type, vpiHandle scope, int index) {
    vpiHandle it = vpi_iterate(type, scope), h = NULL;
    for (int i = 0; it && i <= index; ++i) h = vpi_scan(it);
    if (h && it) vpi_release_handle(it);
    return h;
}

static vpiHandle multi(const char* label, PLI_INT32 type, vpiHandle from, vpiHandle to) {
    vpiHandle path = vpi_handle_multi(type, from, to);
    PLI_INT32 level = vpi_chk_error(NULL);
    if (path) vpi_printf("%s: %s level=%d\n", label, vpi_get_str(vpiType, path), (int)level);
    else vpi_printf("%s: NULL level=%d\n", label, (int)level);
    return path;
}

static PLI_INT32 d18(PLI_BYTE8* data) {
    (void)data;
    vpiHandle s = vpi_handle_by_name((PLI_BYTE8*)"tb.s", NULL);
    vpiHandle k = vpi_handle_by_name((PLI_BYTE8*)"tb.k", NULL);
    vpiHandle sa = nth(vpiPort, s, 0), so = nth(vpiPort, s, 1), sov = nth(vpiPort, s, 2);
    vpiHandle ki = nth(vpiPort, k, 0), kv = nth(vpiPort, k, 1);
    vpiHandle path = multi("s.o to k.i", vpiInterModPath, so, ki);
    multi("k.i to s.o", vpiInterModPath, ki, so);
    multi("s.a to k.i", vpiInterModPath, sa, ki);
    multi("k.i to k.i", vpiInterModPath, ki, ki);
    multi("s.ov to k.i", vpiInterModPath, sov, ki);
    multi("s.o to NULL", vpiInterModPath, so, NULL);
    multi("vpiModPath", vpiModPath, so, ki);
    multi("s.ov to k.v", vpiInterModPath, sov, kv);
    multi("s.ov[0] to k.v[0]", vpiInterModPath, nth(vpiBit, sov, 1), nth(vpiBit, kv, 1));
    vpiHandle again = vpi_handle_multi(vpiInterModPath, so, ki);
    vpi_printf("same ports compare: %d\n", (int)vpi_compare_objects(path, again));
    vpiHandle ports = vpi_iterate(vpiPort, path), port;
    while (ports && (port = vpi_scan(ports)) != NULL)
        vpi_printf("port: %s\n", vpi_get_str(vpiFullName, port));
    get("path", path, 3);
    static const double delays[] = {2, 3, 4};
    put("put path 2 3 4", path, delays, 3);
    get("path", path, 3);
    get("path 2", path, 2);
    get("k.i", ki, 3);
    get("path 1", path, 1);
    put("put path 1", path, delays, 1);
    const char* name = vpi_get_str(vpiName, path);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("vpiName: %s level=%d\n", name ? name : "NULL", (int)level);
    vpiHandle module = vpi_handle(vpiModule, path);
    level = vpi_chk_error(NULL);
    vpi_printf("vpiModule: %s level=%d\n", module ? "handle" : "NULL", (int)level);
    vpiHandle expr = vpi_handle(vpiDelay, path);
    level = vpi_chk_error(NULL);
    vpi_printf("vpiDelay: %s level=%d\n", expr ? "handle" : "NULL", (int)level);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d18";
    task.calltf = d18;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
