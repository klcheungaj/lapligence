/* Companion of S43-D12_startup_before_initializers.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static int calls;

static int read_int(const char* name) {
    vpiHandle object = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    s_vpi_value value;
    if (!object) return -1;
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    vpi_get_value(object, &value);
    return (int)value.value.integer;
}

static void show(const char* when) {
    vpi_printf("%s: k=%d y=%d p=%d\n", when, read_int("tb.k"), read_int("tb.y"),
               read_int("pkg::p"));
}

static PLI_INT32 order_compiletf(PLI_BYTE8* data) {
    (void)data;
    vpi_printf("compiletf\n");
    return 0;
}

static PLI_INT32 order_calltf(PLI_BYTE8* data) {
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle argument = arguments ? vpi_scan(arguments) : NULL;
    s_vpi_value value;
    int which = -1;
    (void)data;
    if (argument) {
        memset(&value, 0, sizeof(value));
        value.format = vpiIntVal;
        vpi_get_value(argument, &value);
        which = (int)value.value.integer;
        vpi_free_object(arguments);
    }
    ++calls;
    vpi_printf("calltf %d (call %d)\n", which, calls);
    /* The design variables initialized so far: tb.k precedes tb.y. */
    if (which == 2) vpi_printf("  k=%d p=%d\n", read_int("tb.k"), read_int("pkg::p"));
    memset(&value, 0, sizeof(value));
    value.format = vpiIntVal;
    value.value.integer = calls * 10;
    vpi_put_value(call, &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 action(p_cb_data data) {
    show(data->reason == cbEndOfCompile ? "cbEndOfCompile" : "cbStartOfSimulation");
    return 0;
}

static void add_action(PLI_INT32 reason) {
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = reason;
    cb.cb_rtn = action;
    vpi_register_cb(&cb);
}

static void startup(void) {
    s_vpi_systf_data systf;
    vpi_printf("startup\n");
    memset(&systf, 0, sizeof(systf));
    systf.type = vpiSysFunc;
    systf.sysfunctype = vpiIntFunc;
    systf.tfname = (PLI_BYTE8*)"$order";
    systf.compiletf = order_compiletf;
    systf.calltf = order_calltf;
    vpi_register_systf(&systf);
    add_action(cbEndOfCompile);
    add_action(cbStartOfSimulation);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
