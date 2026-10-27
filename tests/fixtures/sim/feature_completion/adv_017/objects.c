/* ADV-017: static assertion information (IEEE 1800-2009 39.3) and
 * callbacks that control their own assertion (39.4.2, 39.5.2). Companion
 * of objects.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static vpiHandle r1;
static vpiHandle self_removing;
static int self_removing_calls;

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

static void describe(vpiHandle a) {
    vpiHandle module = vpi_handle(vpiModule, a);
    vpi_printf("%s name=%s full=%s file=%s line=%d column=%d deferred=%d module=%s def=%s\n",
               (const char*)vpi_get_str(vpiType, a), text((const char*)vpi_get_str(vpiName, a)),
               text((const char*)vpi_get_str(vpiFullName, a)),
               base_name((const char*)vpi_get_str(vpiFile, a)), (int)vpi_get(vpiLineNo, a),
               (int)vpi_get(vpiColumn, a), (int)vpi_get(vpiIsDeferred, a),
               module ? (const char*)vpi_get_str(vpiFullName, module) : "NULL",
               text((const char*)vpi_get_str(vpiDefName, a)));
}

static void list(const char* label, vpiHandle scope) {
    vpi_printf("assertions of %s:\n", label);
    vpiHandle iterator = vpi_iterate(vpiAssertion, scope), a;
    while (iterator && (a = vpi_scan(iterator))) {
        vpi_printf("  ");
        describe(a);
    }
}

static PLI_INT32 trace(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                       p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)reason;
    (void)assertion;
    if (info)
        vpi_printf("%u r1 %s attempt=%u\n", (unsigned)time->low, (const char*)user_data,
                   (unsigned)info->attemptStartTime.low);
    else
        vpi_printf("%u r1 %s info=NULL\n", (unsigned)time->low, (const char*)user_data);
    return 0;
}

/* Kills the attempt it is told about when it began at 15. */
static PLI_INT32 killer(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                        p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)reason;
    (void)time;
    (void)user_data;
    if (info->attemptStartTime.low != 15) return 0;
    PLI_INT32 result = vpi_control(vpiAssertionKill, assertion, &info->attemptStartTime);
    vpi_printf("15 killer: kill attempt 15 -> %d\n", (int)result);
    return 0;
}

/* Removes its own record on its second call. */
static PLI_INT32 remover(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                         p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)reason;
    (void)assertion;
    (void)user_data;
    self_removing_calls++;
    vpi_printf("%u remover call %d attempt=%u\n", (unsigned)time->low, self_removing_calls,
               (unsigned)info->attemptStartTime.low);
    if (self_removing_calls == 2)
        vpi_printf("%u remover removes itself -> %d\n", (unsigned)time->low,
                   (int)vpi_remove_cb(self_removing));
    return 0;
}

static PLI_INT32 objects(PLI_BYTE8* data) {
    (void)data;
    list("the design", NULL);
    list("tb.u_leaf", vpi_handle_by_name((PLI_BYTE8*)"tb.u_leaf", NULL));
    list("tb.u_chk", vpi_handle_by_name((PLI_BYTE8*)"tb.u_chk", NULL));

    r1 = vpi_handle_by_name((PLI_BYTE8*)"tb.r1", NULL);
    vpiHandle k_x = vpi_handle_by_name((PLI_BYTE8*)"k_x", vpi_handle_by_name((PLI_BYTE8*)"tb.u_chk", NULL));
    vpi_printf("by name k_x below tb.u_chk: %s\n", k_x ? (const char*)vpi_get_str(vpiFullName, k_x) : "NULL");
    vpi_printf("relations of r1:\n");
    status("vpiClockingEvent", vpi_handle(vpiClockingEvent, r1) != NULL);
    status("vpiDisableCondition", vpi_handle(vpiDisableCondition, r1) != NULL);
    status("vpiProperty", vpi_handle(vpiProperty, r1) != NULL);
    status("vpiEndLine", (int)vpi_get(vpiEndLine, r1));
    status("vpiScope is tb", vpi_compare_objects(vpi_handle(vpiScope, r1),
                                                 vpi_handle_by_name((PLI_BYTE8*)"tb", NULL)));

    vpi_register_assertion_cb(r1, cbAssertionStart, trace, (PLI_BYTE8*)"Start");
    vpi_register_assertion_cb(r1, cbAssertionStart, killer, NULL);
    self_removing = vpi_register_assertion_cb(r1, cbAssertionStart, remover, NULL);
    vpi_register_assertion_cb(r1, cbAssertionSuccess, trace, (PLI_BYTE8*)"Success");
    vpi_register_assertion_cb(r1, cbAssertionFailure, trace, (PLI_BYTE8*)"Failure");
    vpi_register_assertion_cb(r1, cbAssertionKill, trace, (PLI_BYTE8*)"Kill");
    return 0;
}

static PLI_INT32 summary(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("r1 attempts=%d success=%d killed=%d covered=%d\n",
               (int)vpi_get(vpiAssertAttemptCovered, r1), (int)vpi_get(vpiAssertSuccessCovered, r1),
               (int)vpi_get(vpiAssertKillCovered, r1), (int)vpi_get(vpiCovered, r1));
    vpi_printf("remover ran %d times\n", self_removing_calls);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$objects";
    task.calltf = objects;
    vpi_register_systf(&task);
    task.tfname = (PLI_BYTE8*)"$summary";
    task.calltf = summary;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
