#include "vpi_user.h"
#include <string.h>

static int packed_value(vpiHandle handle, char expected) {
    if (!handle || vpi_get(vpiSize, handle) != 129) return 0;
    s_vpi_value value = {0};
    value.format = vpiBinStrVal;
    vpi_get_value(handle, &value);
    if (vpi_chk_error(NULL) || !value.value.str || strlen(value.value.str) != 129) return 0;
    for (int bit = 0; bit < 129; ++bit)
        if (value.value.str[bit] != expected) return 0;
    return 1;
}

static PLI_INT32 probe(p_cb_data callback) {
    (void)callback;
    vpiHandle array = vpi_handle_by_name((PLI_BYTE8*)"tb.r", NULL);
    vpiHandle peer = vpi_handle_by_name((PLI_BYTE8*)"tb.peer", NULL);
    if (!array || vpi_get(vpiType, array) != vpiRegArray ||
        vpi_get(vpiSize, array) != 129 ||
        strcmp(vpi_get_str(vpiName, array), "r") ||
        strcmp(vpi_get_str(vpiFullName, array), "tb.r") ||
        !peer || vpi_get(vpiType, peer) != vpiNet || !packed_value(peer, '1')) return 1;
    vpi_printf("vpi partitioned net shapes ok\n");
    vpi_release_handle(array);
    vpi_release_handle(peer);
    return 0;
}

static void register_probe(void) {
    s_cb_data end = {0};
    end.reason = cbEndOfSimulation;
    end.cb_rtn = probe;
    (void)vpi_register_cb(&end);
}

void (*vlog_startup_routines[])(void) = {register_probe, NULL};
