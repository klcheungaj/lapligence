/* Companion of A19-D9_modpath_objects.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <string.h>

static const char* name_of(vpiHandle h) {
    const char* name = h ? (const char*)vpi_get_str(vpiFullName, h) : NULL;
    return name ? name : "NULL";
}

static void terms(PLI_INT32 type, const char* label, vpiHandle path) {
    vpiHandle it = vpi_iterate(type, path), term;
    while (it && (term = vpi_scan(it)))
        vpi_printf(" %s(%s dir=%d edge=0x%02x)", label, name_of(vpi_handle(vpiExpr, term)),
                   (int)vpi_get(vpiDirection, term), (int)vpi_get(vpiEdge, term));
}

static void objects(void) {
    vpiHandle it = vpi_iterate(vpiModPath, vpi_handle_by_name((PLI_BYTE8*)"tb.u", NULL)), path;
    int index = 0;
    while (it && (path = vpi_scan(it))) {
        const char* name = (const char*)vpi_get_str(vpiName, path);
        vpi_printf("path %d: name=%s type=%d polarity=%d data_polarity=%d has_ifnone=%d", index++,
                   name ? name : "NULL", (int)vpi_get(vpiPathType, path),
                   (int)vpi_get(vpiPolarity, path), (int)vpi_get(vpiDataPolarity, path),
                   (int)vpi_get(vpiModPathHasIfNone, path));
        terms(vpiModPathIn, "in", path);
        terms(vpiModPathOut, "out", path);
        vpiHandle data_term = vpi_handle(vpiModDataPathIn, path);
        vpi_printf(" data=%s", data_term ? name_of(vpi_handle(vpiExpr, data_term)) : "NULL");
        vpi_printf(" condition=%s\n", name_of(vpi_handle(vpiCondition, path)));
    }
}

static void delays(vpiHandle path, double* rise, double* fall, int put) {
    s_vpi_time times[2];
    s_vpi_delay delay;
    memset(times, 0, sizeof(times));
    memset(&delay, 0, sizeof(delay));
    delay.da = times;
    delay.no_of_delays = 2;
    delay.time_type = vpiScaledRealTime;
    times[0].type = times[1].type = vpiScaledRealTime;
    times[0].real = *rise;
    times[1].real = *fall;
    if (put) vpi_put_delays(path, &delay);
    else vpi_get_delays(path, &delay);
    *rise = times[0].real;
    *fall = times[1].real;
}

/* The paths of tb.p: the terminals (one each) and delays. */
static void pairs(const char* label) {
    vpiHandle it = vpi_iterate(vpiModPath, vpi_handle_by_name((PLI_BYTE8*)"tb.p", NULL)), path;
    int index = 0;
    while (it && (path = vpi_scan(it))) {
        vpiHandle in = vpi_iterate(vpiModPathIn, path), out = vpi_iterate(vpiModPathOut, path);
        double rise = 0, fall = 0;
        delays(path, &rise, &fall, 0);
        vpi_printf("%s %d: %s polarity=%d has_ifnone=%d condition=%s", label, index++,
                   name_of(path), (int)vpi_get(vpiPolarity, path),
                   (int)vpi_get(vpiModPathHasIfNone, path), name_of(vpi_handle(vpiCondition, path)));
        vpi_printf(" in=%s", name_of(vpi_handle(vpiExpr, vpi_scan(in))));
        vpi_printf(" out=%s", name_of(vpi_handle(vpiExpr, vpi_scan(out))));
        vpi_printf(" more=%d delays=%g,%g\n", vpi_scan(in) || vpi_scan(out), rise, fall);
    }
}

/* Write the first path of tb.p with this name. */
static void put(const char* name, double rise, double fall) {
    vpiHandle it = vpi_iterate(vpiModPath, vpi_handle_by_name((PLI_BYTE8*)"tb.p", NULL)), path;
    while (it && (path = vpi_scan(it))) {
        if (strcmp(name_of(path), name)) continue;
        delays(path, &rise, &fall, 1);
        vpi_free_object(it);
        return;
    }
}

static PLI_INT32 d9(PLI_BYTE8* data) {
    (void)data;
    objects();
    pairs("pair");
    put("tb.p.b$z", 20, 21);
    put("tb.p.c$y", 22, 23);
    pairs("after");
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d9";
    task.calltf = d9;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
