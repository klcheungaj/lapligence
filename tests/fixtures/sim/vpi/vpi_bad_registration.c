/* H28 negative VPI plugin: malformed registration and callback reason. */
#include "vpi_user.h"

static PLI_INT32 bad_call(PLI_BYTE8* user_data) {
    (void)user_data;
    return 0;
}

static PLI_INT32 bad_callback(struct t_cb_data* data) {
    (void)data;
    return 0;
}

static void register_bad(void) {
    s_vpi_systf_data bad = {0};
    bad.type = vpiSysFunc;
    bad.sysfunctype = 99;
    bad.tfname = (PLI_BYTE8*)"$vpi_bad";
    bad.calltf = bad_call;
    vpiHandle handle = vpi_register_systf(&bad);
    s_vpi_error_info error = {0};
    if (!handle && vpi_chk_error(&error))
        vpi_printf("bad-registration=%s\n", error.code);

    s_cb_data callback = {0};
    callback.reason = 999;
    callback.cb_rtn = bad_callback;
    (void)vpi_register_cb(&callback);
    if (vpi_chk_error(&error))
        vpi_printf("bad-callback=%s\n", error.code);

    (void)vpi_get(vpiType, (vpiHandle)(uintptr_t)1);
    if (vpi_chk_error(&error))
        vpi_printf("bad-handle=%s\n", error.code);
}

void (*vlog_startup_routines[])(void) = {register_bad, NULL};
