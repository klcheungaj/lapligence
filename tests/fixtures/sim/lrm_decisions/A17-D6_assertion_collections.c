/* Decision A17-D6: collections in assertion system control. See
 * A17-D6_assertion_collections.sv. */
#include "sv_vpi_user.h"
#include <string.h>

static void status(const char* what, int ok) {
    s_vpi_error_info error;
    memset(&error, 0, sizeof(error));
    int level = vpi_chk_error(&error);
    vpi_printf("%s -> %s%s%s\n", what, ok ? "ok" : "refused", level ? " " : "",
               level ? (const char*)error.code : "");
}

static PLI_INT32 system_cb(p_cb_data data) {
    vpi_printf("%u cbAssertionSysOff\n", (unsigned)data->time->low);
    return 0;
}

static PLI_INT32 d6(PLI_BYTE8* user_data) {
    (void)user_data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    s_vpi_value value = {vpiIntVal, {0}};
    vpi_get_value(vpi_scan(arguments), &value);
    vpi_release_handle(arguments);
    if (value.value.integer == 0) {
        s_cb_data cb;
        s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
        memset(&cb, 0, sizeof(cb));
        cb.reason = cbAssertionSysOff;
        cb.cb_rtn = system_cb;
        cb.time = &time;
        vpi_register_cb(&cb);
        vpiHandle c = vpi_create(vpiCollection, NULL, NULL);
        c = vpi_create(vpiCollection, c, vpi_handle_by_name((PLI_BYTE8*)"tb.u1", NULL));
        c = vpi_create(vpiCollection, c, vpi_handle_by_name((PLI_BYTE8*)"tb.t1", NULL));
        vpi_printf("collection type=%s\n", (const char*)vpi_get_str(vpiType, c));
        vpiHandle it = vpi_iterate(vpiMember, c), m;
        while ((m = vpi_scan(it)))
            vpi_printf("  member %s %s\n", (const char*)vpi_get_str(vpiType, m),
                       (const char*)vpi_get_str(vpiFullName, m));
        status("add a net", vpi_create(vpiCollection, c, vpi_handle_by_name((PLI_BYTE8*)"tb.clk",
                                                                           NULL)) != NULL);
        status("create a vpiModule", vpi_create(vpiModule, NULL, NULL) != NULL);
        status("SysOff on the collection", vpi_control(vpiAssertionSysOff, c));
        vpi_release_handle(c);
        return 0;
    }
    static const char* names[] = {"tb.t1", "tb.t2", "tb.u1.k", "tb.u2.k"};
    for (int i = 0; i < 4; ++i) {
        vpiHandle a = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        vpi_printf("%s attempts=%d\n", names[i], (int)vpi_get(vpiAssertAttemptCovered, a));
    }
    vpi_control(vpiFinish, 0);
    return 0;
}

static void startup(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof(tf));
    tf.type = vpiSysTask;
    tf.tfname = "$d6";
    tf.calltf = d6;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
