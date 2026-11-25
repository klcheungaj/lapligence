/* A callback of +reason takes the action +do: it ends the run, removes the
 * application's callbacks, registers more of them or writes values. */
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "vpi_user.h"
#include "sv_vpi_user.h"

static char reason_name[32], action_name[32];
static vpiHandle handles[64];
static int32_t handle_count, fired;

typedef struct {
    const char* name;
    PLI_INT32 reason;
    const char* object;
    PLI_UINT32 delay;
} trigger_t;

static const trigger_t triggers[] = {
    {"value", cbValueChange, "tb.v", 0},
    {"valueq", cbValueChange, "tb.q", 0},
    {"valuey", cbValueChange, "tb.y", 0},
    {"valuedyn", cbValueChange, "tb.dyn", 0},
    {"size", cbSizeChange, "tb.dyn", 0},
    {"readwrite", cbReadWriteSynch, NULL, 3},
    {"readwrite0", cbReadWriteSynch, NULL, 0},
    {"readonly", cbReadOnlySynch, NULL, 3},
    {"readonly0", cbReadOnlySynch, NULL, 0},
    {"nba", cbNBASynch, NULL, 5},
    {"after", cbAfterDelay, NULL, 3},
    {"after0", cbAfterDelay, NULL, 0},
    {"next", cbNextSimTime, NULL, 0},
    {"start", cbAtStartOfSimTime, NULL, 5},
    {"end", cbAtEndOfSimTime, NULL, 5},
    {"force", cbForce, "tb.v", 0},
    {"release", cbRelease, "tb.v", 0},
    {"disable", cbDisable, "tb.w", 0},
    {"thread", cbStartOfThread, NULL, 0},
    {"error", cbError, NULL, 0},
    {"plierror", cbPLIError, NULL, 0},
    {"endofsim", cbEndOfSimulation, NULL, 0},
};

static int32_t is(const char* text, const char* name) { return !strcmp(text, name); }

static void read_arguments(void) {
    s_vpi_vlog_info info;
    reason_name[0] = action_name[0] = 0;
    vpi_get_vlog_info(&info);
    for (int32_t i = 0; i < info.argc; i++) {
        if (!strncmp(info.argv[i], "+reason=", 8))
            snprintf(reason_name, sizeof reason_name, "%s", info.argv[i] + 8);
        if (!strncmp(info.argv[i], "+do=", 4))
            snprintf(action_name, sizeof action_name, "%s", info.argv[i] + 4);
    }
}

static void report(const char* what) {
    s_vpi_error_info error;
    if (vpi_chk_error(&error)) vpi_printf("%s failed\n", what);
}

static PLI_INT32 nothing(p_cb_data data) {
    (void)data;
    return 0;
}

static void write_values(void) {
    static const char* names[] = {"tb.v", "tb.q", "tb.a", "tb.clk", "tb.dyn[1]"};
    for (int32_t i = 0; i < 5; i++) {
        vpiHandle object = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        static const PLI_INT32 flags[] = {vpiNoDelay, vpiInertialDelay, vpiForceFlag, vpiReleaseFlag};
        if (!object) continue;
        for (int32_t f = 0; f < 4; f++) {
            s_vpi_value value;
            s_vpi_time time;
            memset(&time, 0, sizeof time);
            time.type = vpiSimTime;
            time.low = f == 1;
            value.format = vpiIntVal;
            value.value.integer = 1;
            vpi_put_value(object, &value, &time, flags[f]);
            report(names[i]);
        }
        vpi_release_handle(object);
    }
}

static void register_many(void) {
    static const PLI_INT32 reasons[] = {cbAfterDelay, cbReadWriteSynch, cbReadOnlySynch, cbNextSimTime,
                                        cbAtStartOfSimTime};
    for (int32_t i = 0; i < 20; i++) {
        s_cb_data data;
        s_vpi_time time;
        memset(&data, 0, sizeof data);
        memset(&time, 0, sizeof time);
        time.type = vpiSimTime;
        time.low = (PLI_UINT32)(i % 3);
        data.reason = reasons[i % 5];
        data.cb_rtn = nothing;
        data.time = &time;
        vpi_register_cb(&data);
        vpi_chk_error(NULL);
    }
}

static void act(const char* where) {
    if (is(action_name, "finish")) vpi_control(vpiFinish, 1);
    if (is(action_name, "write")) write_values();
    if (is(action_name, "register")) register_many();
    if (is(action_name, "remove")) {
        for (int32_t i = 0; i < handle_count; i++)
            vpi_printf("removed %d\n", (int)vpi_remove_cb(handles[i]));
        handle_count = 0;
    }
    vpi_printf("%s: %s done\n", where, action_name);
}

static PLI_INT32 called_back(p_cb_data data) {
    if (++fired > 2) return 0;
    vpi_printf("reason %d\n", (int)data->reason);
    act("callback");
    return 0;
}

static PLI_INT32 assertion_called_back(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                                       p_vpi_attempt_info info, PLI_BYTE8* user) {
    (void)time, (void)assertion, (void)info, (void)user;
    vpi_printf("assertion %d\n", (int)reason);
    act("assertion");
    return 0;
}

static void arm(const trigger_t* trigger) {
    s_cb_data data;
    s_vpi_time time;
    s_vpi_value value;
    memset(&data, 0, sizeof data);
    memset(&time, 0, sizeof time);
    time.type = vpiSimTime;
    time.low = trigger->delay;
    value.format = vpiIntVal;
    data.reason = trigger->reason;
    data.cb_rtn = called_back;
    data.time = &time;
    data.value = &value;
    if (trigger->object) data.obj = vpi_handle_by_name((PLI_BYTE8*)trigger->object, NULL);
    vpiHandle handle = vpi_register_cb(&data);
    report("registration");
    if (handle) handles[handle_count++] = handle;
}

static PLI_INT32 arm_call(PLI_BYTE8* user) {
    (void)user;
    read_arguments();
    if (is(reason_name, "calltf")) act("calltf");
    for (size_t i = 0; i < sizeof triggers / sizeof triggers[0]; i++)
        if (is(reason_name, triggers[i].name)) arm(&triggers[i]);
    if (is(reason_name, "assertion")) {
        vpiHandle assertion, all = vpi_iterate(vpiAssertion, vpi_handle_by_name((PLI_BYTE8*)"tb", NULL));
        while (all && (assertion = vpi_scan(all)))
            vpi_register_assertion_cb(assertion, cbAssertionFailure, assertion_called_back, NULL);
        report("assertion registration");
    }
    return 0;
}

static PLI_INT32 fn_call(PLI_BYTE8* user) {
    s_vpi_value value;
    (void)user;
    value.format = vpiIntVal;
    value.value.integer = 2;
    vpi_put_value(vpi_handle(vpiSysTfCall, NULL), &value, NULL, vpiNoDelay);
    if (is(reason_name, "func")) act("func");
    return 0;
}

static PLI_INT32 started(p_cb_data data) {
    (void)data;
    read_arguments();
    if (is(reason_name, "startofsim")) act("start");
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    s_cb_data data;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$arm";
    task.calltf = arm_call;
    vpi_register_systf(&task);
    task.type = vpiSysFunc;
    task.sysfunctype = vpiIntFunc;
    task.tfname = "$fn";
    task.calltf = fn_call;
    vpi_register_systf(&task);
    memset(&data, 0, sizeof data);
    data.reason = cbStartOfSimulation;
    data.cb_rtn = started;
    vpi_register_cb(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
