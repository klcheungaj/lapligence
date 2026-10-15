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
    /* 37.17 detail 9: vpiSize of an array is its element count. */
    vpiHandle element = array ? vpi_handle_by_index(array, -1) : NULL;
    if (!array || vpi_get(vpiType, array) != vpiNetArray ||
        vpi_get(vpiSize, array) != 2 ||
        strcmp(vpi_get_str(vpiName, array), "r") ||
        strcmp(vpi_get_str(vpiFullName, array), "tb.r") ||
        !element || vpi_get(vpiType, element) != vpiNet ||
        strcmp(vpi_get_str(vpiFullName, element), "tb.r[-1]") ||
        !packed_value(element, '1') ||
        !peer || vpi_get(vpiType, peer) != vpiNet || !packed_value(peer, '1')) return 1;
    vpi_printf("vpi partitioned net shapes ok\n");
    vpi_release_handle(element);
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
