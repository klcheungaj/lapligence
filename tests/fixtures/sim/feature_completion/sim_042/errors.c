/* SIM-042 A03: standard error behaviour and handle lifetime. Companion of
 * errors.sv (and missing.sv, which uses an unregistered task). Every check
 * prints the returned value class and the vpi_chk_error level. */
#include "vpi_user.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static vpiHandle kept_argument;
static vpiHandle kept_iterator;
static vpiHandle kept_call;

static void report(const char* label, int returned_ok) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    PLI_INT32 level = vpi_chk_error(&info);
    /* vpi_chk_error leaves the status unchanged (38.2). */
    PLI_INT32 again = vpi_chk_error(NULL);
    vpi_printf("%s: %s level=%d again=%d code=%s\n", label, returned_ok ? "ok" : "refused",
               (int)level, (int)again, level && info.code ? (const char*)info.code : "-");
}

static vpiHandle named(const char* name) {
    return vpi_handle_by_name((PLI_BYTE8*)name, NULL);
}

static PLI_INT32 errors(PLI_BYTE8* data) {
    (void)data;
    vpiHandle bogus = (vpiHandle)(uintptr_t)0x12345;
    report("get bogus", vpi_get(vpiType, bogus) != vpiUndefined);
    report("get_str bogus", vpi_get_str(vpiName, bogus) != NULL);
    report("get null", vpi_get(vpiSize, NULL) != vpiUndefined);
    vpiHandle v = named("tb.v");
    report("lookup", v != NULL);
    report("get unknown property", vpi_get(9999, v) != vpiUndefined);
    report("get_str defname of reg", vpi_get_str(vpiDefName, v) != NULL);
    report("relation lowconn of reg", vpi_handle(vpiLowConn, v) != NULL);
    report("iterate nets of reg", vpi_iterate(vpiNet, v) != NULL);
    report("iterate nets of tb", vpi_iterate(vpiNet, named("tb")) != NULL);
    report("iterate params of tb", vpi_iterate(vpiParameter, named("tb")) != NULL);
    report("iterate modules of tb (none)", vpi_iterate(vpiModule, named("tb")) != NULL);
    report("lookup missing", named("tb.nothing") != NULL);
    report("size of real", vpi_get(vpiSize, named("tb.r")) != vpiUndefined);
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    /* A real has no scalar representation (S42-D9). */
    value.format = vpiScalarVal;
    vpi_get_value(named("tb.r"), &value);
    report("scalar of real", 0);
    value.format = 77;
    vpi_get_value(v, &value);
    report("unknown format", 0);
    vpiHandle mem = named("tb.mem");
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    vpi_get_value(mem, &value);
    report("value of whole array", 0);
    report("index out of range", vpi_handle_by_index(mem, -1) != NULL);
    PLI_INT32 indices[3] = {0, 0, 0};
    report("too many indices", vpi_handle_by_multi_index(mem, 3, indices) != NULL);
    report("bit index out of range", vpi_handle_by_index(v, 8) != NULL);
    report("part select reversed", named("tb.v[2:5]") != NULL);
    value.format = vpiIntVal;
    value.value.integer = 1;
    report("put to parameter", vpi_put_value(named("tb.P"), &value, NULL, vpiNoDelay) != NULL);
    report("put to net array element",
           vpi_put_value(vpi_handle_by_index(named("tb.n"), 0), &value, NULL, vpiNoDelay) != NULL);
    report("put bad flags", vpi_put_value(v, &value, NULL, 42) != NULL);
    value.format = vpiDecStrVal;
    value.value.str = (PLI_BYTE8*)"12a";
    report("put malformed decimal", vpi_put_value(v, &value, NULL, vpiNoDelay) != NULL);
    report("cancel non-event", vpi_put_value(v, NULL, NULL, vpiCancelEvent) != NULL);
    report("systf call outside a call", vpi_handle(vpiSysTfCall, v) != NULL);

    s_vpi_systf_data bad;
    memset(&bad, 0, sizeof(bad));
    bad.type = vpiSysTask;
    bad.tfname = (PLI_BYTE8*)"$bad-name";
    report("register bad name", vpi_register_systf(&bad) != NULL);
    bad.tfname = (PLI_BYTE8*)"$errors";
    report("register duplicate", vpi_register_systf(&bad) != NULL);
    bad.tfname = (PLI_BYTE8*)"$late_registered";
    bad.type = vpiSysFunc;
    bad.sysfunctype = 99;
    report("register bad function type", vpi_register_systf(&bad) != NULL);

    vpiHandle element = vpi_handle_by_index(mem, 1);
    report("element", element != NULL);
    report("release element", vpi_release_handle(element) == 1);
    report("use released element", vpi_get(vpiType, element) != vpiUndefined);
    report("release twice", vpi_release_handle(element) == 1);
    vpiHandle reused = vpi_handle_by_index(mem, 1);
    vpi_printf("reused handle differs=%d\n", reused != element);
    report("old handle after slot reuse", vpi_get(vpiType, element) != vpiUndefined);
    vpi_release_handle(reused);
    vpiHandle iterator = vpi_iterate(vpiReg, mem);
    int count = 0;
    while (vpi_scan(iterator)) ++count;
    vpi_printf("scanned %d elements\n", count);
    report("scan finished iterator", vpi_scan(iterator) != NULL);
    report("release catalog object", vpi_release_handle(v) == 1);
    report("catalog object stays valid", vpi_get(vpiType, v) != vpiUndefined);
    return 0;
}

static PLI_INT32 keep(PLI_BYTE8* data) {
    (void)data;
    kept_call = vpi_handle(vpiSysTfCall, NULL);
    kept_iterator = vpi_iterate(vpiArgument, kept_call);
    kept_argument = vpi_scan(kept_iterator);
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    vpi_get_value(kept_argument, &value);
    vpi_printf("kept argument type=%d value=%d\n", (int)vpi_get(vpiType, kept_argument),
               (int)value.value.integer);
    value.value.integer = 3;
    report("put to expression argument",
           vpi_put_value(kept_argument, &value, NULL, vpiNoDelay) != NULL);
    /* Every request may share one handle (37.2.1); releasing one copy must
     * leave the other copies usable (37.2.2). */
    vpiHandle copy = vpi_handle(vpiSysTfCall, NULL);
    report("second call request", vpi_compare_objects(copy, kept_call) == 1);
    report("release one call copy", vpi_release_handle(copy) == 1);
    report("other call copy stays valid", vpi_get(vpiType, kept_call) == vpiSysTaskCall);
    vpiHandle arguments = vpi_iterate(vpiArgument, kept_call);
    report("arguments of the other copy", arguments != NULL && vpi_scan(arguments) != NULL);
    /* Releasing a call-owned iterator early must leave the call's other
     * handles (kept_argument, kept_iterator) to expire with the call. */
    report("release a partly scanned iterator", vpi_release_handle(arguments) == 1);
    return 0;
}

static PLI_INT32 use_kept(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value value;
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    vpi_get_value(kept_argument, &value);
    report("argument after its call", 0);
    report("iterator after its call", vpi_scan(kept_iterator) != NULL);
    report("call handle persists", vpi_get(vpiType, kept_call) == vpiSysTaskCall);
    report("call line after a released copy", vpi_get(vpiLineNo, kept_call) == 15);
    report("arguments of an inactive call", vpi_iterate(vpiArgument, kept_call) != NULL);
    return 0;
}

/* Handle churn for the sanitizer lane: every allocation is released or
 * expires with its call, and stale handles are probed after reuse. */
static PLI_INT32 stress(PLI_BYTE8* data) {
    (void)data;
    vpiHandle mem = named("tb.mem");
    vpiHandle v = named("tb.v");
    int stale = 0, live = 0;
    for (int round = 0; round < 20000; ++round) {
        vpiHandle element = vpi_handle_by_index(mem, round % 4);
        vpiHandle bit = vpi_handle_by_index(v, round % 8);
        vpiHandle iterator = vpi_iterate(vpiBit, element);
        vpiHandle first = vpi_scan(iterator);
        live += element && bit && iterator && first;
        vpi_release_handle(first);
        vpi_free_object(iterator);
        vpi_release_handle(bit);
        vpi_release_handle(element);
        stale += vpi_get(vpiType, element) == vpiUndefined && vpi_chk_error(NULL) == vpiError;
        stale += vpi_get(vpiType, first) == vpiUndefined && vpi_chk_error(NULL) == vpiError;
    }
    vpi_printf("stress live=%d stale=%d\n", live, stale);
    return 0;
}

static void register_task(const char* name, PLI_INT32 (*calltf)(PLI_BYTE8*)) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)name;
    task.calltf = calltf;
    if (!vpi_register_systf(&task)) vpi_printf("registration of %s failed\n", name);
}

static void register_all(void) {
    register_task("$errors", errors);
    register_task("$keep", keep);
    register_task("$use_kept", use_kept);
    register_task("$stress", stress);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
