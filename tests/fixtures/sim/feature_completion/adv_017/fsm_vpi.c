/* ADV-017: the FSM objects and FSM coverage operations of IEEE 1800-2009
 * 40.5.2-40.5.4. Companion of fsm_vpi.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* text(const char* value) { return value ? value : "NULL"; }

static const char* base_name(const char* path) {
    const char* slash = path ? strrchr(path, '/') : NULL;
    const char* backslash = path ? strrchr(path, '\\') : NULL;
    if (backslash && (!slash || backslash > slash)) slash = backslash;
    return slash ? slash + 1 : text(path);
}

static void status(const char* what, int value) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    vpi_printf("  %s -> %d level=%d code=%s\n", what, value, (int)level,
               level ? (const char*)info.code : "-");
}

static vpiHandle callback;
static int changes;

static PLI_INT32 changed(p_cb_data data) {
    vpi_printf("  cs changed at %u to %d\n", (unsigned)data->time->low, (int)data->value->value.integer);
    ++changes;
    return 0;
}

static void state_line(vpiHandle state) {
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(state, &value);
    s_vpi_value bits;
    bits.format = vpiBinStrVal;
    vpi_get_value(state, &bits);
    vpi_printf("    %s %s value=%d bits=%s covered=%d max=%d count=%d\n",
               (const char*)vpi_get_str(vpiType, state), text((const char*)vpi_get_str(vpiName, state)),
               (int)value.value.integer, bits.value.str, (int)vpi_get(vpiCovered, state),
               (int)vpi_get(vpiCoverMax, state), (int)vpi_get64(vpiCoveredCount, state));
}

static void describe(vpiHandle fsm, int states) {
    vpiHandle scope = vpi_handle(vpiScope, fsm);
    vpi_printf("  %s name=%s full=%s def=%s file=%s line=%d scope=%s covered=%d max=%d count=%d\n",
               (const char*)vpi_get_str(vpiType, fsm), text((const char*)vpi_get_str(vpiName, fsm)),
               text((const char*)vpi_get_str(vpiFullName, fsm)),
               text((const char*)vpi_get_str(vpiDefName, fsm)),
               base_name((const char*)vpi_get_str(vpiFile, fsm)), (int)vpi_get(vpiLineNo, fsm),
               scope ? (const char*)vpi_get_str(vpiFullName, scope) : "NULL",
               (int)vpi_get(vpiCovered, fsm), (int)vpi_get(vpiCoverMax, fsm),
               (int)vpi_get(vpiCoveredCount, fsm));
    vpiHandle expression = vpi_handle(vpiFsmStateExpression, fsm);
    if (expression)
        vpi_printf("    expression %s %s size=%d\n", (const char*)vpi_get_str(vpiType, expression),
                   text((const char*)vpi_get_str(vpiFullName, expression)),
                   (int)vpi_get(vpiSize, expression));
    else
        status("vpiFsmStateExpression", 0);
    if (!states) return;
    vpiHandle iterator = vpi_iterate(vpiFsmStates, fsm), state;
    while (iterator && (state = vpi_scan(iterator))) {
        state_line(state);
        vpiHandle owner = vpi_handle(vpiFsm, state);
        if (!owner || !vpi_compare_objects(owner, fsm)) vpi_printf("    owner mismatch\n");
        if (owner) vpi_release_handle(owner);
    }
}

static void each_fsm(const char* scope_name, int states) {
    vpiHandle scope = vpi_handle_by_name((PLI_BYTE8*)scope_name, NULL);
    vpi_printf("fsms of %s: state coverage %d\n", scope_name, (int)vpi_get(vpiFsmStateCoverage, scope));
    vpiHandle iterator = vpi_iterate(vpiFsm, scope), fsm;
    while (iterator && (fsm = vpi_scan(iterator))) describe(fsm, states);
}

static PLI_INT32 fsm_objects(PLI_BYTE8* data) {
    (void)data;
    each_fsm("tb.c", 1);
    each_fsm("tb.p", 1);
    each_fsm("tb", 1);
    vpi_printf("errors:\n");
    vpiHandle iterator = vpi_iterate(vpiFsm, NULL);
    status("vpi_iterate(vpiFsm, NULL)", iterator != NULL);
    vpiHandle cs = vpi_handle_by_name("tb.c.cs", NULL);
    iterator = vpi_iterate(vpiFsm, cs);
    status("vpi_iterate(vpiFsm, signal)", iterator != NULL);
    iterator = vpi_iterate(vpiFsmStates, cs);
    status("vpi_iterate(vpiFsmStates, signal)", iterator != NULL);
    vpiHandle fsm = vpi_scan(vpi_iterate(vpiFsm, vpi_handle_by_name("tb.c", NULL)));
    status("vpi_get(vpiFsmStateCoverage, fsm)", (int)vpi_get(vpiFsmStateCoverage, fsm));
    status("vpi_control(vpiCoverageStop, vpiFsmStateCoverage, fsm)",
           (int)vpi_control(vpiCoverageStop, vpiFsmStateCoverage, fsm));
    s_vpi_value value;
    value.format = vpiIntVal;
    value.value.integer = 1;
    vpiHandle state = vpi_scan(vpi_iterate(vpiFsmStates, fsm));
    vpi_put_value(state, &value, NULL, vpiNoDelay);
    status("vpi_put_value(state)", 0);
    s_cb_data cb;
    s_vpi_time time;
    s_vpi_value cb_value;
    memset(&cb, 0, sizeof(cb));
    time.type = vpiSimTime;
    cb_value.format = vpiIntVal;
    cb.reason = cbValueChange;
    cb.cb_rtn = changed;
    cb.obj = cs;
    cb.time = &time;
    cb.value = &cb_value;
    callback = vpi_register_cb(&cb);
    return 0;
}

static PLI_INT32 fsm_counts(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle argument = vpi_scan(vpi_iterate(vpiArgument, call));
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(argument, &value);
    vpi_printf("counts %d:\n", (int)value.value.integer);
    if (value.value.integer == 1 && callback) {
        /* The FSM keeps watching the state vector after the callback goes. */
        vpi_printf("  callback saw %d changes, removed %d\n", changes, (int)vpi_remove_cb(callback));
        callback = NULL;
    }
    each_fsm("tb.c", 1);
    each_fsm("tb.p", 0);
    return 0;
}

static PLI_INT32 fsm_control(PLI_BYTE8* data) {
    (void)data;
    vpiHandle c = vpi_handle_by_name("tb.c", NULL);
    vpiHandle tb = vpi_handle_by_name("tb", NULL);
    vpi_printf("control:\n");
    status("check c", (int)vpi_control(vpiCoverageCheck, vpiFsmStateCoverage, c));
    status("check tb", (int)vpi_control(vpiCoverageCheck, vpiFsmStateCoverage, tb));
    status("stop c", (int)vpi_control(vpiCoverageStop, vpiFsmStateCoverage, c));
    status("reset p", (int)vpi_control(vpiCoverageReset, vpiFsmStateCoverage,
                                       vpi_handle_by_name("tb.p", NULL)));
    return 0;
}

static PLI_INT32 fsm_files(PLI_BYTE8* data) {
    (void)data;
    vpiHandle c = vpi_handle_by_name("tb.c", NULL);
    vpi_printf("files:\n");
    status("save", (int)vpi_control(vpiCoverageSave, vpiFsmStateCoverage, "fsm_vpi.db"));
    status("start c", (int)vpi_control(vpiCoverageStart, vpiFsmStateCoverage, c));
    status("merge", (int)vpi_control(vpiCoverageMerge, vpiFsmStateCoverage, "fsm_vpi.db"));
    return 0;
}

static void define(const char* name, PLI_INT32 (*routine)(PLI_BYTE8*)) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)name;
    task.calltf = routine;
    vpi_register_systf(&task);
}

static void startup(void) {
    define("$fsm_objects", fsm_objects);
    define("$fsm_counts", fsm_counts);
    define("$fsm_control", fsm_control);
    define("$fsm_files", fsm_files);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
