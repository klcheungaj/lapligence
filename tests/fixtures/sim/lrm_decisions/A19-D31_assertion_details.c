/* Decision A19-D31: the location, instance, module and disable condition
 * of sequence and property instances, and the concurrent assertions of a
 * scope (IEEE 1800-2009 37.10, 37.12, 37.45, 37.52, 37.80). See
 * A19-D31_assertion_details.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

static vpiHandle get(const char* name) { return vpi_handle_by_name((PLI_BYTE8*)name, NULL); }

static const char* text(int property, vpiHandle h) {
    const char* s = h ? vpi_get_str(property, h) : NULL;
    return level() || !s ? "-" : s;
}

static void relation(const char* label, int type, vpiHandle h) {
    vpiHandle r = vpi_handle(type, h);
    int error = level();
    if (!r)
        vpi_printf("  %s NULL level=%d\n", label, error);
    else
        vpi_printf("  %s type=%d %s\n", label, (int)vpi_get(vpiType, r), text(vpiFullName, r));
}

static void property(const char* label, int property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    int error = level();
    if (error)
        vpi_printf(" %s=error%d", label, error);
    else
        vpi_printf(" %s=%d", label, value);
}

static void instance(const char* label, vpiHandle h) {
    vpi_printf("%s type=%d name=%s", label, (int)vpi_get(vpiType, h), text(vpiName, h));
    property("start", vpiStartLine, h);
    property("column", vpiColumn, h);
    property("end", vpiEndLine, h);
    property("endcolumn", vpiEndColumn, h);
    property("covered", vpiCovered, h);
    property("attempts", vpiAssertAttemptCovered, h);
    vpi_printf(" def=%s\n", text(vpiDefName, h));
    relation("instance", vpiInstance, h);
    relation("module", vpiModule, h);
    if ((int)vpi_get(vpiType, h) == vpiPropertyInst) relation("disable", vpiDisableCondition, h);
}

static void iterate(const char* label, int type, vpiHandle scope) {
    vpiHandle it = vpi_iterate(type, scope), h;
    int error = level();
    vpi_printf("%s:", label);
    if (!it) vpi_printf(" NULL level=%d", error);
    while (it && (h = vpi_scan(it))) vpi_printf(" %s", text(vpiFullName, h));
    vpi_printf("\n");
}

static PLI_INT32 d31(PLI_BYTE8* data) {
    (void)data;
    const char* names[] = {"tb.ap1", "tb.ap2", "tb.cs1", "tb.g.ga", NULL};
    for (int i = 0; names[i]; ++i) {
        vpiHandle a = get(names[i]);
        vpi_printf("%s type=%d\n", text(vpiFullName, a), (int)vpi_get(vpiType, a));
        relation("instance", vpiInstance, a);
        relation("module", vpiModule, a);
        vpiHandle p = vpi_handle(vpiProperty, a);
        level();
        if (p && (int)vpi_get(vpiType, p) == vpiPropertySpec) {
            p = vpi_handle(vpiPropertyExpr, p);
            level();
        }
        if (p && ((int)vpi_get(vpiType, p) == vpiPropertyInst ||
                  (int)vpi_get(vpiType, p) == vpiSequenceInst))
            instance("  inst", p);
    }
    iterate("tb concurrent", vpiConcurrentAssertions, get("tb"));
    iterate("g concurrent", vpiConcurrentAssertions, get("tb.g"));
    iterate("blk concurrent", vpiConcurrentAssertions, get("tb.blk"));
    iterate("g assertions", vpiAssertion, get("tb.g"));
    return 0;
}

static void register_tasks(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d31";
    task.calltf = d31;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_tasks, 0};
