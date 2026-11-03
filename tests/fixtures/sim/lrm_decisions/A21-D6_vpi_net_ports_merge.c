/* Decision A21-D6: net ports of a model reached through VPI merge (IEEE
 * 1800-2009 23.3.3.7, 38.34, 37.14). See A21-D6_vpi_net_ports_merge.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* names[] = {"tb.wn", "tb.u.w", "tb.clk", "tb.u.c",
                              "tb.u.o", "tb.on", "tb.u.q", "tb.qn"};
#define N_NAMES 8
static int changes[N_NAMES];

static vpiHandle by_name(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static PLI_INT32 changed(p_cb_data data) {
    changes[*(int*)data->user_data]++;
    return 0;
}

static void put(const char* name, int value, int flags) {
    s_vpi_value v;
    v.format = vpiScalarVal;
    v.value.scalar = value;
    vpi_put_value(by_name(name), &v, NULL, flags);
}

static vpiHandle port_w(void) {
    vpiHandle ports = vpi_iterate(vpiPort, by_name("tb.u"));
    vpiHandle found = NULL;
    for (vpiHandle port; ports && (port = vpi_scan(ports));)
        if (!found && strcmp(vpi_get_str(vpiName, port), "w") == 0) found = port;
    return found;
}

static void delays(void) {
    s_vpi_error_info info;
    s_vpi_time times[3];
    s_vpi_delay delay;
    memset(times, 0, sizeof times);
    memset(&delay, 0, sizeof delay);
    for (int i = 0; i < 3; ++i) {
        times[i].type = vpiScaledRealTime;
        times[i].real = 3.0;
    }
    delay.da = times;
    delay.no_of_delays = 3;
    delay.time_type = vpiScaledRealTime;
    vpiHandle port = port_w();
    vpi_put_delays(port, &delay);
    (void)vpi_chk_error(&info);
    memset(times, 0, sizeof times);
    vpi_get_delays(port, &delay);
    int refused = vpi_chk_error(&info) && info.level >= vpiError;
    vpi_printf("get_delays of port w: %s, rise %g\n", refused ? "refused" : "read",
               times[0].real);
}

static PLI_INT32 d6(PLI_BYTE8* user) {
    (void)user;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = args ? vpi_scan(args) : NULL;
    s_vpi_value v;
    v.format = vpiIntVal;
    vpi_get_value(arg, &v);
    if (args && arg) vpi_free_object(args);
    switch (v.value.integer) {
        case 0: {
            static int index[N_NAMES];
            for (int i = 0; i < N_NAMES; ++i) {
                static s_vpi_time time;
                static s_vpi_value value;
                s_cb_data data;
                time.type = vpiSuppressTime;
                value.format = vpiSuppressVal;
                index[i] = i;
                memset(&data, 0, sizeof data);
                data.reason = cbValueChange;
                data.cb_rtn = changed;
                data.obj = by_name(names[i]);
                data.time = &time;
                data.value = &value;
                data.user_data = (PLI_BYTE8*)&index[i];
                vpi_register_cb(&data);
            }
            break;
        }
        case 1:
            put("tb.u.w", vpi0, vpiNoDelay);
            put("tb.on", vpi0, vpiNoDelay);
            put("tb.u.c", vpi0, vpiNoDelay);
            put("tb.qn", vpi0, vpiNoDelay);
            break;
        case 2:
            put("tb.wn", vpi1, vpiNoDelay);
            put("tb.u.o", vpi1, vpiNoDelay);
            break;
        case 3:
            put("tb.u.w", vpi0, vpiForceFlag);
            put("tb.on", vpi0, vpiForceFlag);
            put("tb.u.c", vpi0, vpiForceFlag);
            put("tb.qn", vpi0, vpiForceFlag);
            break;
        case 4:
            put("tb.wn", vpi0, vpiReleaseFlag);
            put("tb.u.o", vpi0, vpiReleaseFlag);
            put("tb.u.c", vpi0, vpiReleaseFlag);
            put("tb.qn", vpi0, vpiReleaseFlag);
            break;
        case 5:
            delays();
            break;
        default:
            for (int i = 0; i < N_NAMES; ++i)
                vpi_printf("changes of %s: %d\n", names[i], changes[i]);
            break;
    }
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d6";
    task.calltf = d6;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, 0};
