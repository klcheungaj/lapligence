/* Companion of S43-D9_postponed_systf_read_only.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

/* $bump(v): returns v + 10 and tries y = v + 20. */
static PLI_INT32 bump(PLI_BYTE8* user) {
    (void)user;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle argument = vpi_scan(arguments);
    vpi_free_object(arguments);
    s_vpi_value v;
    v.format = vpiIntVal;
    vpi_get_value(argument, &v);
    s_vpi_value y;
    y.format = vpiIntVal;
    y.value.integer = v.value.integer + 20;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.y", NULL), &y, NULL, vpiNoDelay);
    vpi_printf("write y: error %d\n", (int)vpi_chk_error(NULL));
    v.value.integer += 10;
    vpi_put_value(call, &v, NULL, vpiNoDelay);
    vpi_printf("set result: error %d\n", (int)vpi_chk_error(NULL));
    return 0;
}

static void startup(void) {
    s_vpi_systf_data tf;
    memset(&tf, 0, sizeof(tf));
    tf.type = vpiSysFunc;
    tf.sysfunctype = vpiIntFunc;
    tf.tfname = (PLI_BYTE8*)"$bump";
    tf.calltf = bump;
    vpi_register_systf(&tf);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
