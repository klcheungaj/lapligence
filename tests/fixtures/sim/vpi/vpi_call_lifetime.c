/* R22: enforce the bridge's documented callback-borrow lifetime policy. */
#include "vpi_user.h"
#include <string.h>

typedef struct {
    vpiHandle call;
    vpiHandle iterator;
    vpiHandle argument;
} borrowed_handles;

static borrowed_handles compiled, sized, executed;
static vpiHandle persistent_scope;
static int calls;

static int capture(borrowed_handles *handles) {
    handles->call = vpi_handle(vpiSysTfCall, NULL);
    handles->iterator = vpi_iterate(vpiArgument, handles->call);
    handles->argument = vpi_scan(handles->iterator);
    return handles->call && handles->iterator && handles->argument &&
           vpi_get(vpiSize, handles->argument) == 8 && !vpi_chk_error(NULL);
}

static int handle_error(void) {
    s_vpi_error_info error = {0};
    return vpi_chk_error(&error) && error.code &&
           strcmp((const char *)error.code, "LLG_VPI_HANDLE") == 0;
}

static int stale(const borrowed_handles *handles) {
    (void)vpi_get(vpiType, handles->call);
    if (!handle_error()) return 0;
    (void)vpi_get(vpiSize, handles->argument);
    if (!handle_error()) return 0;
    s_vpi_value value = {0};
    value.format = vpiIntVal;
    vpi_get_value(handles->argument, &value);
    if (!handle_error()) return 0;
    vpiHandle item = vpi_scan(handles->iterator);
    if (item || !handle_error()) return 0;
    return 1;
}

static PLI_INT32 compile_call(PLI_BYTE8 *data) {
    (void)data;
    persistent_scope = vpi_handle_by_name((PLI_BYTE8 *)"tb", NULL);
    return persistent_scope && capture(&compiled) ? 0 : 1;
}

static PLI_INT32 size_call(PLI_BYTE8 *data) {
    (void)data;
    if (!stale(&compiled) || !capture(&sized)) return 0;
    return 8;
}

static PLI_INT32 run_call(PLI_BYTE8 *data) {
    (void)data;
    if (!stale(&sized) || (calls && !stale(&executed)) ||
        vpi_get(vpiType, persistent_scope) != vpiModule ||
        !capture(&executed)) return 1;
    s_vpi_value value = {0};
    value.format = vpiIntVal;
    value.value.integer = 17;
    (void)vpi_put_value(executed.call, &value, NULL, vpiNoDelay);
    if (vpi_chk_error(NULL)) return 1;
    vpi_printf("vpi borrowed handles ok %d\n", ++calls);
    return 0;
}

static void register_probe(void) {
    s_vpi_systf_data function = {0};
    function.type = vpiSysFunc;
    function.sysfunctype = vpiSizedFunc;
    function.tfname = (PLI_BYTE8 *)"$vpi_lifetime";
    function.compiletf = compile_call;
    function.sizetf = size_call;
    function.calltf = run_call;
    (void)vpi_register_systf(&function);
}

void (*vlog_startup_routines[])(void) = {register_probe, NULL};
