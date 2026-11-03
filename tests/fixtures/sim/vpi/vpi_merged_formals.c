#include "vpi_user.h"
#include <string.h>

/* A net named `name` of `size` bits whose binary value is `expected`. */
static int net_value(const char* name, PLI_INT32 size, const char* expected) {
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    if (!handle) return 0;
    s_vpi_value value = {0};
    value.format = vpiBinStrVal;
    vpi_get_value(handle, &value);
    int ok = vpi_get(vpiType, handle) == vpiNet && vpi_get(vpiSize, handle) == size &&
             !strcmp(vpi_get_str(vpiFullName, handle), name) && !vpi_chk_error(NULL) &&
             value.value.str && !strcmp(value.value.str, expected);
    vpi_release_handle(handle);
    return ok;
}

static PLI_INT32 probe(p_cb_data callback) {
    (void)callback;
    if (!net_value("tb.v", 4, "0110") || !net_value("tb.u.a", 1, "1") ||
        !net_value("tb.u.p", 2, "01") || !net_value("tb.u.q", 2, "00") ||
        !net_value("tb.w", 4, "z1zz") || !net_value("tb.u.o", 1, "1"))
        return 1;
    vpi_printf("vpi merged formals ok\n");
    return 0;
}

static void register_probe(void) {
    s_cb_data end = {0};
    end.reason = cbEndOfSimulation;
    end.cb_rtn = probe;
    (void)vpi_register_cb(&end);
}

void (*vlog_startup_routines[])(void) = {register_probe, NULL};
