/* Decision A19-D30: clocking blocks, clocking io declarations, the default
 * and global clocking and the default disable condition of an instance
 * (IEEE 1800-2009 37.5, 37.9, 37.44). See A19-D30_clocking_blocks.sv. */
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

static void object(const char* label, vpiHandle h) {
    int error = level();
    if (!h)
        vpi_printf("%s NULL level=%d\n", label, error);
    else
        vpi_printf("%s type=%d %s level=%d\n", label, (int)vpi_get(vpiType, h),
                   text(vpiFullName, h), error);
}

static vpiHandle relation(const char* label, int type, vpiHandle h) {
    vpiHandle r = vpi_handle(type, h);
    object(label, r);
    return r;
}

static void property(const char* label, int property, vpiHandle h) {
    int value = (int)vpi_get(property, h);
    int error = level();
    if (error)
        vpi_printf("%s error=%d\n", label, error);
    else
        vpi_printf("%s %d\n", label, value);
}

static void iteration(const char* label, int type, vpiHandle h) {
    vpiHandle it = vpi_iterate(type, h), x;
    int error = level();
    vpi_printf("%s", label);
    if (!it) vpi_printf(" NULL");
    while (it && (x = vpi_scan(it)))
        vpi_printf(" %d:%s", (int)vpi_get(vpiType, x), text(vpiName, x));
    vpi_printf(" level=%d\n", error);
}

static void skew(const char* label, int type, vpiHandle h) {
    vpiHandle s = vpi_handle(type, h);
    int error = level();
    if (!s) {
        vpi_printf("%s NULL level=%d\n", label, error);
        return;
    }
    vpiHandle delay = vpi_handle(vpiDelay, s);
    int delay_error = level();
    long value = -1;
    if (delay) {
        s_vpi_value v;
        v.format = vpiIntVal;
        vpi_get_value(delay, &v);
        value = (long)v.value.integer;
    }
    vpi_printf("%s type=%d delay=%ld level=%d\n", label, (int)vpi_get(vpiType, s), value,
               delay_error);
}

static void io(vpiHandle d) {
    const char* name = text(vpiName, d);
    vpi_printf("io %s direction=%d input edge=%d output edge=%d\n", name,
               (int)vpi_get(vpiDirection, d), (int)vpi_get(vpiInputEdge, d),
               (int)vpi_get(vpiOutputEdge, d));
    skew("  input skew", vpiInputSkew, d);
    skew("  output skew", vpiOutputSkew, d);
    vpiHandle e = vpi_handle(vpiExpr, d);
    vpi_printf("  expr type=%d %s level=%d\n", e ? (int)vpi_get(vpiType, e) : 0,
               e ? text(vpiDecompile, e) : "NULL", level());
    relation("  vpiClockingBlock", vpiClockingBlock, d);
    relation("  vpiScope", vpiScope, d);
    relation("  vpiInstance", vpiInstance, d);
    relation("  vpiParent", vpiParent, d);
    property("  vpiSize", vpiSize, d);
}

static PLI_INT32 d30(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = get("tb");
    relation("tb vpiDefaultClocking", vpiDefaultClocking, tb);
    relation("tb vpiGlobalClocking", vpiGlobalClocking, tb);
    vpiHandle dd = vpi_handle(vpiDefaultDisableIff, tb);
    vpi_printf("tb vpiDefaultDisableIff %s level=%d\n", dd ? text(vpiDecompile, dd) : "NULL",
               level());
    iteration("tb clocking blocks", vpiClockingBlock, tb);
    iteration("tb internal scopes", vpiInternalScope, tb);
    vpiHandle cb = get("tb.cb");
    object("tb.cb", cb);
    property("cb vpiInputEdge", vpiInputEdge, cb);
    property("cb vpiOutputEdge", vpiOutputEdge, cb);
    skew("cb input skew", vpiInputSkew, cb);
    skew("cb output skew", vpiOutputSkew, cb);
    vpiHandle ev = vpi_handle(vpiClockingEvent, cb);
    vpi_printf("cb clocking event type=%d level=%d\n", ev ? (int)vpi_get(vpiType, ev) : 0,
               level());
    relation("cb vpiInstance", vpiInstance, cb);
    relation("cb vpiModule", vpiModule, cb);
    property("cb vpiTimeUnit", vpiTimeUnit, cb);
    iteration("cb io decls", vpiClockingIODecl, cb);
    iteration("cb property decls", vpiPropertyDecl, cb);
    iteration("cb variables", vpiVariables, cb);
    vpiHandle it = vpi_iterate(vpiClockingIODecl, cb), d;
    while (it && (d = vpi_scan(it))) io(d);
    object("tb.cb.r by name", get("tb.cb.r"));
    vpiHandle p = get("tb.p");
    relation("tb.p vpiDefaultClocking", vpiDefaultClocking, p);
    relation("tb.p vpiDefaultDisableIff", vpiDefaultDisableIff, p);
    vpiHandle pcb = get("tb.p.pcb");
    property("pcb vpiInputEdge", vpiInputEdge, pcb);
    property("pcb vpiOutputEdge", vpiOutputEdge, pcb);
    relation("pcb vpiInstance", vpiInstance, pcb);
    vpiHandle as = vpi_iterate(vpiAssertion, tb), a;
    while (as && (a = vpi_scan(as))) relation("assertion vpiClockingBlock", vpiClockingBlock, a);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$d30";
    task.calltf = d30;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
