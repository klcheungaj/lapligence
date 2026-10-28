/* Decision A17-D10 (see A17-D10_system_start.sv). */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static unsigned now_low(void) {
    s_vpi_time now = {vpiSimTime, 0, 0, 0.0};
    vpi_get_time(NULL, &now);
    return (unsigned)now.low;
}

static PLI_INT32 on_system(p_cb_data data) {
    vpi_printf("%u %s placed %s\n", now_low(),
               data->reason == cbAssertionSysInitialized ? "SysInitialized" : "SysOn",
               (const char*)data->user_data);
    return 0;
}

static void place(const char* label) {
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    const PLI_INT32 reasons[] = {cbAssertionSysInitialized, cbAssertionSysOn};
    for (int i = 0; i < 2; ++i) {
        s_cb_data cb;
        memset(&cb, 0, sizeof(cb));
        cb.reason = reasons[i];
        cb.cb_rtn = on_system;
        cb.time = &time;
        cb.user_data = (PLI_BYTE8*)label;
        if (!vpi_register_cb(&cb)) vpi_printf("%u place %s failed\n", now_low(), label);
    }
}

static PLI_INT32 at_start(p_cb_data data) {
    (void)data;
    vpi_printf("%u cbStartOfSimulation\n", now_low());
    place("at the start of simulation");
    return 0;
}

static PLI_INT32 task(PLI_BYTE8* data) {
    vpi_printf("%u place %s\n", now_low(), (const char*)data);
    place((const char*)data);
    return 0;
}

static void registration(void) {
    static const char* names[] = {"$d10_initial", "$d10_woken", "$d10_zero", "$d10_late"};
    static const char* labels[] = {"in an initial procedure", "in a woken always procedure",
                                   "after #0", "at time 1"};
    for (int i = 0; i < 4; ++i) {
        s_vpi_systf_data t;
        memset(&t, 0, sizeof(t));
        t.type = vpiSysTask;
        t.tfname = (PLI_BYTE8*)names[i];
        t.calltf = task;
        t.user_data = (PLI_BYTE8*)labels[i];
        vpi_register_systf(&t);
    }
    place("by a startup routine");
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbStartOfSimulation;
    cb.cb_rtn = at_start;
    vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
