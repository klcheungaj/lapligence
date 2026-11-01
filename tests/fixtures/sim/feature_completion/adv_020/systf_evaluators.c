/* ADV-020: a user system function called from evaluated contexts. Companion
 * of systf_evaluators.sv. */
#include "vpi_user.h"

static PLI_INT32 plus1(PLI_BYTE8* data) {
    (void)data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle arguments = vpi_iterate(vpiArgument, call);
    vpiHandle argument = vpi_scan(arguments);
    vpi_free_object(arguments);
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(argument, &value);
    value.value.integer += 1;
    vpi_put_value(call, &value, NULL, vpiNoDelay);
    return 0;
}

static PLI_INT32 size(PLI_BYTE8* data) {
    (void)data;
    return 32;
}

static void register_plus1(void) {
    s_vpi_systf_data systf = {vpiSysFunc, vpiIntFunc, (PLI_BYTE8*)"$plus1", plus1, NULL, size,
                              NULL};
    vpi_register_systf(&systf);
}

void (*vlog_startup_routines[])(void) = {register_plus1, NULL};
