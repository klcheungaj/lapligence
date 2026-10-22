/* SIM-043: re-entrant registration and removal, callback handle lifetime. */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle second, doomed, keep;
static int first_calls, second_calls, third_calls;

static unsigned now_of(p_cb_data data) { return data->time ? data->time->low : 0u; }

static const char* error_code(void) {
    static char code[64];
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    if (!vpi_chk_error(&info)) return "none";
    snprintf(code, sizeof(code), "%s", info.code ? (const char*)info.code : "?");
    return code;
}

static vpiHandle watch(vpiHandle object, PLI_INT32 (*routine)(p_cb_data), const char* label) {
    s_vpi_time time;
    s_vpi_value value;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    time.type = vpiSimTime;
    value.format = vpiIntVal;
    data.reason = cbValueChange;
    data.cb_rtn = routine;
    data.obj = object;
    data.time = &time;
    data.value = &value;
    data.user_data = (PLI_BYTE8*)label;
    return vpi_register_cb(&data);
}

static vpiHandle after(unsigned delay, PLI_INT32 (*routine)(p_cb_data), const char* label) {
    s_vpi_time time;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    time.type = vpiSimTime;
    time.high = 0;
    time.low = delay;
    data.reason = cbAfterDelay;
    data.cb_rtn = routine;
    data.time = &time;
    data.user_data = (PLI_BYTE8*)label;
    return vpi_register_cb(&data);
}

static PLI_INT32 third(p_cb_data data) {
    ++third_calls;
    vpi_printf("[%u] %s v=%d\n", now_of(data), data->user_data, (int)data->value->value.integer);
    return 0;
}

static PLI_INT32 second_cb(p_cb_data data) {
    ++second_calls;
    vpi_printf("[%u] %s v=%d\n", now_of(data), data->user_data, (int)data->value->value.integer);
    return 0;
}

/* Runs before `second` on every change of v: at v=2 it removes `second`,
 * which then misses that change although it was due in the same dispatch,
 * and registers `third`, which first sees v=3. */
static PLI_INT32 first(p_cb_data data) {
    ++first_calls;
    int v = (int)data->value->value.integer;
    vpi_printf("[%u] %s v=%d\n", now_of(data), data->user_data, v);
    if (v == 2) {
        PLI_INT32 removed = vpi_remove_cb(second);
        PLI_INT32 again = vpi_remove_cb(second);
        const char* code = error_code();
        vpi_printf("  remove second -> %d, again -> %d %s\n", (int)removed, (int)again, code);
        watch(data->obj, third, "third");
    }
    return 0;
}

static PLI_INT32 never(p_cb_data data) {
    vpi_printf("[%u] %s: unreachable\n", now_of(data), data->user_data);
    return 0;
}

static PLI_INT32 canceller(p_cb_data data) {
    PLI_INT32 removed = vpi_remove_cb(doomed);
    vpi_printf("[%u] %s: remove pending time callback -> %d\n", now_of(data), data->user_data,
               (int)removed);
    s_cb_data info;
    memset(&info, 0, sizeof(info));
    vpi_get_cb_info(keep, &info);
    const char* code = error_code();
    vpi_printf("  info of a fired one-shot -> %s reason=%d user=%s\n", code, (int)info.reason,
               info.user_data ? info.user_data : "-");
    removed = vpi_remove_cb(keep);
    code = error_code();
    vpi_printf("  remove the fired one-shot -> %d %s\n", (int)removed, code);
    vpi_get_cb_info(keep, &info);
    vpi_printf("  info after removal -> %s\n", error_code());
    /* Registrations of an action that already occurred never run, but
     * their handles stay valid; past the runtime's bound of 65536 kept
     * handles the oldest ones are freed. */
    enum { FLOOD = 65536 + 8 };
    vpiHandle oldest = NULL, newest = NULL;
    for (int i = 0; i < FLOOD; ++i) {
        s_cb_data late;
        memset(&late, 0, sizeof(late));
        late.reason = cbEndOfCompile;
        late.cb_rtn = never;
        late.user_data = (PLI_BYTE8*)"late";
        newest = vpi_register_cb(&late);
        if (!oldest) oldest = newest;
    }
    vpi_get_cb_info(oldest, &info);
    code = error_code();
    vpi_printf("  oldest of %d late actions -> %s\n", (int)FLOOD, code);
    memset(&info, 0, sizeof(info));
    vpi_get_cb_info(newest, &info);
    code = error_code();
    vpi_printf("  newest -> %s reason=%d\n", code, (int)info.reason);
    return 0;
}

static PLI_INT32 kept(p_cb_data data) {
    vpi_printf("[%u] %s: runs although its handle was released\n", now_of(data), data->user_data);
    return 0;
}

static void try_reason(PLI_INT32 reason, vpiHandle object, PLI_INT32 time_type,
                       const char* label) {
    s_vpi_time time;
    s_vpi_value value;
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    memset(&time, 0, sizeof(time));
    time.type = time_type;
    value.format = vpiIntVal;
    data.reason = reason;
    data.cb_rtn = never;
    data.obj = object;
    data.time = &time;
    data.value = &value;
    vpiHandle handle = vpi_register_cb(&data);
    const char* code = error_code();
    vpi_printf("%s -> %s %s\n", label, handle ? "handle" : "null", code);
}

static PLI_INT32 started(p_cb_data data) {
    (void)data;
    vpiHandle v = vpi_handle_by_name("tb.v", NULL);
    watch(v, first, "first");
    second = watch(v, second_cb, "second");
    s_cb_data info;
    memset(&info, 0, sizeof(info));
    vpi_get_cb_info(second, &info);
    vpi_printf("info: reason=%d user=%s value format=%d\n", (int)info.reason, info.user_data,
               (int)info.value->format);
    keep = after(2, kept, "kept");
    vpi_printf("release kept handle -> %d\n", (int)vpi_release_handle(keep));
    keep = after(1, kept, "kept-1");
    doomed = after(4, never, "doomed");
    after(3, canceller, "canceller");
    try_reason(cbStmt, v, vpiSimTime, "cbStmt");
    try_reason(cbForce, v, vpiSimTime, "cbForce");
    try_reason(cbValueChange, vpi_handle_by_name("tb.m", NULL), vpiSimTime, "whole array");
    try_reason(cbValueChange, vpi_handle_by_name("tb.P", NULL), vpiSimTime, "parameter");
    try_reason(cbValueChange, NULL, vpiSimTime, "null object");
    try_reason(cbAfterDelay, NULL, vpiSuppressTime, "suppressed time");
    return 0;
}

static PLI_INT32 ended(p_cb_data data) {
    vpi_printf("[%u] end: first=%d second=%d third=%d\n", now_of(data), first_calls, second_calls,
               third_calls);
    return 0;
}

static void startup(void) {
    s_cb_data data;
    memset(&data, 0, sizeof(data));
    data.reason = cbStartOfSimulation;
    data.cb_rtn = started;
    vpi_register_cb(&data);
    data.reason = cbEndOfSimulation;
    data.cb_rtn = ended;
    vpi_register_cb(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
