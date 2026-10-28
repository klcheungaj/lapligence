/* Decision A17-D4: assertion relations. See A17-D4_assertion_relations.sv. */
#include "sv_vpi_user.h"
#include <string.h>

static void show(const char* role, vpiHandle h) {
    if (!h) {
        vpi_printf("  %s: NULL\n", role);
        return;
    }
    const char* text = vpi_get_str(vpiDecompile, h);
    vpi_printf("  %s: %s %s\n", role, (const char*)vpi_get_str(vpiType, h),
               text ? text : "(not decompiled)");
    vpi_chk_error(NULL);
}

static PLI_INT32 failed(PLI_INT32 reason, p_vpi_time time, vpiHandle assertion,
                        p_vpi_attempt_info info, PLI_BYTE8* user_data) {
    (void)reason;
    (void)assertion;
    vpi_printf("%u %s failure\n", (unsigned)time->low, (const char*)user_data);
    show("failExpr", info->detail.failExpr);
    return 0;
}

static PLI_INT32 d4(PLI_BYTE8* data) {
    (void)data;
    static const char* names[] = {"tb.q1", "tb.q2", "tb.q3", "tb.q4", "tb.q5"};
    for (int i = 0; i < 5; ++i) {
        vpiHandle a = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        vpi_printf("%s end=%d:%d", names[i], (int)vpi_get(vpiEndLine, a),
                   (int)vpi_get(vpiEndColumn, a));
        if (i < 4) vpi_printf(" inferred=%d", (int)vpi_get(vpiIsClockInferred, a));
        vpi_printf("\n");
        if (i < 4) {
            show("vpiClockingEvent", vpi_handle(vpiClockingEvent, a));
            show("vpiDisableCondition", vpi_handle(vpiDisableCondition, a));
            vpiHandle p = vpi_handle(vpiProperty, a);
            show("vpiProperty", p);
            if (vpi_get(vpiType, p) == vpiPropertySpec) {
                show("spec vpiClockingEvent", vpi_handle(vpiClockingEvent, p));
                show("spec vpiPropertyExpr", vpi_handle(vpiPropertyExpr, p));
            }
        } else {
            show("vpiExpr", vpi_handle(vpiExpr, a));
        }
        show("vpiStmt", vpi_handle(vpiStmt, a));
        show("vpiElseStmt", vpi_handle(vpiElseStmt, a));
        vpi_register_assertion_cb(a, cbAssertionFailure, failed, (PLI_BYTE8*)names[i]);
    }
    return 0;
}

static void startup(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof(tf));
    tf.type = vpiSysTask;
    tf.tfname = "$d4";
    tf.calltf = d4;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
