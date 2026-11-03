/* Registration at start-up reaches no design object; the first call of the
 * task does. */
#include "vpi_user.h"

static vpiHandle registered;

static PLI_INT32 on_change(p_cb_data data) {
    vpi_printf("change %s=%d at %u\n", vpi_get_str(vpiName, data->obj),
               (int)data->value->value.integer, (unsigned)data->time->low);
    return 0;
}

static PLI_INT32 probe(PLI_BYTE8* user_data) {
    (void)user_data;
    s_vpi_systf_data info = {0};
    vpi_get_systf_info(registered, &info);
    vpi_printf("probe %s\n", info.tfname);
    vpiHandle a = vpi_handle_by_name((PLI_BYTE8*)"tb.a", NULL);
    vpiHandle w = vpi_handle_by_name((PLI_BYTE8*)"tb.w", NULL);
    static s_vpi_time time = {vpiSimTime, 0, 0, 0.0};
    static s_vpi_value value = {vpiIntVal, {0}};
    s_cb_data callback = {0};
    callback.reason = cbValueChange;
    callback.obj = w;
    callback.cb_rtn = on_change;
    callback.time = &time;
    callback.value = &value;
    vpiHandle cb = vpi_register_cb(&callback);
    s_vpi_value put = {vpiIntVal, {0}};
    put.value.integer = 5;
    vpi_put_value(a, &put, NULL, vpiNoDelay);
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle task = call ? vpi_handle(vpiUserSystf, call) : NULL;
    vpi_printf("handles=%d callback=%d same-task=%d\n", a && w, cb != NULL,
               task && vpi_compare_objects(task, registered));
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task = {0};
    task.type = vpiSysTask;
    task.tfname = (PLI_BYTE8*)"$lazy_probe";
    task.calltf = probe;
    registered = vpi_register_systf(&task);
    s_vpi_systf_data info = {0};
    vpi_get_systf_info(registered, &info);
    vpi_printf("registered %s systf=%d\n", info.tfname,
               vpi_get(vpiType, registered) == vpiUserSystf);
    s_vpi_error_info error = {0};
    (void)vpi_get(vpiType, (vpiHandle)(uintptr_t)1);
    if (vpi_chk_error(&error)) vpi_printf("unindexed-handle=%s\n", error.code);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
