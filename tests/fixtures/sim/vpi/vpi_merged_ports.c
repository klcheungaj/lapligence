#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static int stage;
static int sink_changes;

static vpiHandle by_name(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static int value_of(const char* name) {
    s_vpi_value value;
    value.format = vpiScalarVal;
    vpi_get_value(by_name(name), &value);
    return value.value.scalar == vpi1 ? 1 : value.value.scalar == vpi0 ? 0 : 2;
}

static void put(const char* name, int scalar, int flags) {
    s_vpi_value value;
    value.format = vpiScalarVal;
    value.value.scalar = scalar;
    vpi_put_value(by_name(name), &value, NULL, flags);
}

static PLI_INT32 changed(p_cb_data data) {
    (void)data;
    ++sink_changes;
    return 0;
}

static vpiHandle port_w(void) {
    vpiHandle ports = vpi_iterate(vpiPort, by_name("tb.u"));
    vpiHandle found = NULL;
    for (vpiHandle port; ports && (port = vpi_scan(ports));)
        if (!found && strcmp(vpi_get_str(vpiName, port), "w") == 0) found = port;
    return found;
}

static PLI_INT32 ports_check(PLI_BYTE8* user) {
    (void)user;
    if (stage++ == 0) {
        static s_vpi_time time;
        static s_vpi_value value;
        s_cb_data data;
        memset(&data, 0, sizeof data);
        time.type = vpiSuppressTime;
        value.format = vpiSuppressVal;
        data.reason = cbValueChange;
        data.cb_rtn = changed;
        data.obj = by_name("tb.u.w");
        data.time = &time;
        data.value = &value;
        vpi_register_cb(&data);
        put("tb.on", vpi1, vpiNoDelay);
        vpi_printf("put on: on=%d u.o=%d\n", value_of("tb.on"), value_of("tb.u.o"));
        put("tb.u.c", vpi1, vpiNoDelay);
        vpi_printf("put u.c: clk=%d u.c=%d\n", value_of("tb.clk"), value_of("tb.u.c"));
        return 0;
    }
    vpi_printf("changes of u.w: %d\n", sink_changes);
    put("tb.u.w", vpi0, vpiNoDelay);
    vpi_printf("put u.w: wn=%d u.w=%d\n", value_of("tb.wn"), value_of("tb.u.w"));
    put("tb.u.w", vpi0, vpiForceFlag);
    vpi_printf("force u.w: wn=%d u.w=%d\n", value_of("tb.wn"), value_of("tb.u.w"));
    put("tb.u.w", vpi0, vpiReleaseFlag);
    vpi_printf("release u.w: wn=%d u.w=%d\n", value_of("tb.wn"), value_of("tb.u.w"));
    s_vpi_time times[3];
    s_vpi_delay delay;
    s_vpi_error_info info;
    memset(times, 0, sizeof times);
    memset(&delay, 0, sizeof delay);
    for (int i = 0; i < 3; ++i) {
        times[i].type = vpiSimTime;
        times[i].low = 3;
    }
    delay.da = times;
    delay.no_of_delays = 3;
    delay.time_type = vpiSimTime;
    vpi_put_delays(port_w(), &delay);
    int put_error = vpi_chk_error(&info) ? (int)info.level : 0;
    memset(times, 0, sizeof times);
    vpi_get_delays(port_w(), &delay);
    int get_error = vpi_chk_error(&info) ? (int)info.level : 0;
    vpi_printf("port w delays: put error %d, get error %d, rise %u\n", put_error, get_error,
               (unsigned)times[0].low);
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$ports_check";
    task.calltf = ports_check;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, 0};
