/* ADV-017: assertion relations (IEEE 1800-2009 37.45-37.51), sequence and
 * property instances and declarations (37.47-37.50), failExpr and instance
 * callbacks (39.4.2). Companion of relations.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static const char* text(const char* value) { return value ? value : "NULL"; }

/* Prints the pending error, if any, and clears it. */
static void status(const char* what) {
    s_vpi_error_info info;
    memset(&info, 0, sizeof(info));
    if (vpi_chk_error(&info)) vpi_printf("    %s: error %s\n", what, (const char*)info.code);
}

static void object(const char* role, vpiHandle h) {
    if (!h) {
        vpi_printf("    %s: NULL\n", role);
        status(role);
        return;
    }
    const char* decompile = vpi_get_str(vpiDecompile, h);
    vpi_chk_error(NULL);
    vpi_printf("    %s: %s line=%d decompile=%s\n", role, (const char*)vpi_get_str(vpiType, h),
               (int)vpi_get(vpiLineNo, h), text(decompile));
}

static PLI_INT32 failed(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                        p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)reason;
    (void)assertion;
    vpi_printf("%u %s failure attempt=%u\n", (unsigned)time->low, (const char*)user_data,
               (unsigned)info->attemptStartTime.low);
    object("failExpr", info->detail.failExpr);
    return 0;
}

static PLI_INT32 instance_cb(PLI_INT32 reason, p_vpi_time time, vpiHandle h,
                             p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    const char* name = reason == cbAssertionStart     ? "start"
                       : reason == cbAssertionSuccess ? "success"
                                                      : "failure";
    vpi_printf("%u %s %s %s attempt=%u\n", (unsigned)time->low, (const char*)user_data, name,
               (const char*)vpi_get_str(vpiType, h), (unsigned)info->attemptStartTime.low);
    return 0;
}

/* The arguments, declaration and formals of an instance; places its
 * callbacks when `label` is not NULL. */
static void instance(vpiHandle h, const char* label) {
    vpi_printf("    instance %s name=%s\n", (const char*)vpi_get_str(vpiType, h),
               (const char*)vpi_get_str(vpiName, h));
    vpiHandle it = vpi_iterate(vpiArgument, h), x;
    while (it && (x = vpi_scan(it))) object("  vpiArgument", x);
    int sequence = vpi_get(vpiType, h) == vpiSequenceInst;
    vpiHandle d = vpi_handle(sequence ? vpiSequenceDecl : vpiPropertyDecl, h);
    status("declaration");
    if (d) {
        vpi_printf("      declaration %s %s line=%d\n", (const char*)vpi_get_str(vpiType, d),
                   (const char*)vpi_get_str(vpiFullName, d), (int)vpi_get(vpiLineNo, d));
        vpiHandle fi = vpi_iterate(sequence ? vpiSeqFormalDecl : vpiPropFormalDecl, d), f;
        while (fi && (f = vpi_scan(fi))) {
            vpiHandle ts = vpi_handle(vpiTypespec, f);
            vpi_printf("        formal %s %s direction=%d typespec=%s\n",
                       (const char*)vpi_get_str(vpiType, f), (const char*)vpi_get_str(vpiName, f),
                       (int)vpi_get(vpiDirection, f),
                       ts ? (const char*)vpi_get_str(vpiType, ts) : "NULL");
            (void)vpi_handle(vpiExpr, f);
            status("formal vpiExpr");
        }
        (void)vpi_iterate(vpiVariables, d);
        status("declaration vpiVariables");
    }
    if (!label) return;
    vpi_register_assertion_cb(h, cbAssertionStart, instance_cb, (PLI_BYTE8*)label);
    vpi_register_assertion_cb(h, cbAssertionSuccess, instance_cb, (PLI_BYTE8*)label);
    vpi_register_assertion_cb(h, cbAssertionFailure, instance_cb, (PLI_BYTE8*)label);
    status("instance callbacks");
    (void)vpi_register_assertion_cb(h, cbAssertionVacuousSuccess, instance_cb, (PLI_BYTE8*)label);
    status("instance vacuous callback");
    (void)vpi_control(vpiAssertionDisable, h);
    status("instance control");
}

static void assertion(vpiHandle a) {
    const char* name = vpi_get_str(vpiName, a);
    vpi_printf("%s %s end=%d:%d", (const char*)vpi_get_str(vpiType, a),
               text((const char*)vpi_get_str(vpiFullName, a)), (int)vpi_get(vpiEndLine, a),
               (int)vpi_get(vpiEndColumn, a));
    PLI_INT32 inferred = vpi_get(vpiIsClockInferred, a);
    vpi_chk_error(NULL);
    PLI_INT32 cover_sequence = vpi_get(vpiIsCoverSequence, a);
    vpi_chk_error(NULL);
    vpi_printf(" inferred=%d cover_sequence=%d\n", (int)inferred, (int)cover_sequence);
    static const int relations[] = {vpiClockingEvent, vpiDisableCondition, vpiProperty,
                                    vpiExpr,          vpiStmt,             vpiElseStmt};
    static const char* names[] = {"vpiClockingEvent", "vpiDisableCondition", "vpiProperty",
                                  "vpiExpr",          "vpiStmt",             "vpiElseStmt"};
    for (int i = 0; i < 6; ++i) {
        vpiHandle h = vpi_handle(relations[i], a);
        object(names[i], h);
        if (!h || relations[i] != vpiProperty) continue;
        if (vpi_get(vpiType, h) == vpiPropertySpec) {
            object("  spec vpiClockingEvent", vpi_handle(vpiClockingEvent, h));
            object("  spec vpiDisableCondition", vpi_handle(vpiDisableCondition, h));
            vpiHandle e = vpi_handle(vpiPropertyExpr, h);
            object("  spec vpiPropertyExpr", e);
            if (e && vpi_get(vpiType, e) == vpiSequenceInst)
                instance(e, !strcmp(name, "c_seq") ? "c_seq.s1" : NULL);
        } else {
            instance(h, !strcmp(name, "a_inst") ? "a_inst.p1" : NULL);
        }
    }
    if (!strcmp(name, "a_spec") || !strcmp(name, "a_inst") || !strcmp(name, "i1"))
        vpi_register_assertion_cb(a, cbAssertionFailure, failed, (PLI_BYTE8*)name);
}

static PLI_INT32 relations(PLI_BYTE8* data) {
    (void)data;
    vpiHandle it = vpi_iterate(vpiAssertion, NULL), a;
    while (it && (a = vpi_scan(it))) assertion(a);
    vpiHandle top = vpi_handle_by_name("tb", NULL), d;
    vpiHandle di = vpi_iterate(vpiSequenceDecl, top);
    while (di && (d = vpi_scan(di)))
        vpi_printf("tb declares %s\n", (const char*)vpi_get_str(vpiFullName, d));
    di = vpi_iterate(vpiPropertyDecl, top);
    while (di && (d = vpi_scan(di)))
        vpi_printf("tb declares %s\n", (const char*)vpi_get_str(vpiFullName, d));
    di = vpi_iterate(vpiSequenceDecl, vpi_handle_by_name("tb.u_leaf", NULL));
    vpi_printf("tb.u_leaf declares %s\n", di ? "something" : "nothing");
    status("tb.u_leaf declarations");
    /* A sequence instance passed to a property instance is nested: it
     * raises no callbacks (decision A17-D5). */
    vpiHandle p2 = vpi_handle(vpiProperty, vpi_handle_by_name("tb.a_nest", NULL));
    vpiHandle arguments = vpi_iterate(vpiArgument, p2);
    vpiHandle nested = arguments ? vpi_scan(arguments) : NULL;
    if (arguments) vpi_release_handle(arguments);
    vpi_printf("nested %s\n", nested ? (const char*)vpi_get_str(vpiType, nested) : "NULL");
    if (nested) instance(nested, NULL);
    vpiHandle callback =
        vpi_register_assertion_cb(nested, cbAssertionStart, instance_cb, (PLI_BYTE8*)"nested");
    status("nested callback");
    vpi_printf("nested callback -> %s\n", callback ? "handle" : "NULL");
    return 0;
}

static void startup(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof(tf));
    tf.type = vpiSysTask;
    tf.tfname = "$relations";
    tf.calltf = relations;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
