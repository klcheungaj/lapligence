/* Companion of S42-D12_port_order.sv: a VPI application (vlog_startup_routines). */
#include "vpi_user.h"
#include <stdio.h>
#include <string.h>

/* Every port of `scope` in iteration order, with its index, and the bits of
 * a vector port; a port whose vpiPortIndex is not its position is
 * reported; each bit prints the index of its parent port. */
static void ports(const char* scope) {
    vpiHandle module = vpi_handle_by_name((PLI_BYTE8*)scope, NULL);
    vpi_printf("%s ports:", scope);
    vpiHandle it = vpi_iterate(vpiPort, module), port;
    PLI_INT32 at = 0;
    while (it && (port = vpi_scan(it)) != NULL) {
        PLI_INT32 index = vpi_get(vpiPortIndex, port);
        PLI_INT32 size = vpi_get(vpiSize, port);
        vpi_printf(" %s[%d]", (const char*)vpi_get_str(vpiName, port), (int)index);
        if (index != at) vpi_printf("(at %d)", (int)at);
        if (size > 1) {
            vpi_printf("{");
            vpiHandle bits = vpi_iterate(vpiBit, port), bit;
            const char* gap = "";
            while (bits && (bit = vpi_scan(bits)) != NULL) {
                vpi_printf("%s%d", gap, (int)vpi_get(vpiPortIndex, vpi_handle(vpiParent, bit)));
                gap = " ";
            }
            vpi_printf("}");
        }
        ++at;
    }
    vpi_printf("\n");
}

static PLI_INT32 port_order(PLI_BYTE8* data) {
    (void)data;
    ports("tb.u1");
    ports("tb.u0");
    ports("tb.u2");
    vpi_printf("tb.st io:");
    vpiHandle it = vpi_iterate(vpiIODecl, vpi_handle_by_name((PLI_BYTE8*)"tb.st", NULL)), io;
    while (it && (io = vpi_scan(it)) != NULL)
        vpi_printf(" %s dir=%d", (const char*)vpi_get_str(vpiName, io), (int)vpi_get(vpiDirection, io));
    vpi_printf("\n");
    return 0;
}

static void register_all(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$port_order";
    data.calltf = port_order;
    if (!vpi_register_systf(&data)) vpi_printf("registration of $port_order failed\n");
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
