/* Decision A19-D93: port bits by index. Companion of
 * A19-D93_port_bits_by_index.sv. */
#include "sv_vpi_user.h"
#include <string.h>

static int failed(void) {
    s_vpi_error_info error;
    return vpi_chk_error(&error) != 0;
}

/* The position of `bit` in the vpiBit iteration of `port`, or -1. */
static int position(vpiHandle port, vpiHandle bit) {
    vpiHandle it = vpi_iterate(vpiBit, port), item;
    int at = 0, found = -1;
    while (it && (item = vpi_scan(it)) != NULL) {
        if (found < 0 && vpi_compare_objects(item, bit)) found = at;
        ++at;
    }
    return found;
}

static void connection(const char* label, PLI_INT32 relation, vpiHandle bit) {
    vpiHandle object = vpi_handle(relation, bit);
    int error = failed();
    vpi_printf(" %s=%s%s", label, object ? (const char*)vpi_get_str(vpiFullName, object) : "NULL",
               error ? " error" : "");
}

/* vpi_handle_by_index of `port` from `low` to `high`: each result's place in
 * the vpiBit iteration (and whether a one-index vpi_handle_by_multi_index
 * gives the same bit), then the connections of every bit. */
static void port_bits(vpiHandle port, int low, int high) {
    vpi_printf("%s size %d:", (const char*)vpi_get_str(vpiName, port), (int)vpi_get(vpiSize, port));
    for (int i = low; i <= high; ++i) {
        PLI_INT32 index = i;
        vpiHandle bit = vpi_handle_by_index(port, i);
        int error = failed();
        vpiHandle multi = vpi_handle_by_multi_index(port, 1, &index);
        int multi_error = failed();
        if (!bit) {
            vpi_printf(" [%d] NULL%s", i, error ? " error" : "");
            continue;
        }
        int same = multi && !multi_error && vpi_compare_objects(bit, multi);
        vpi_printf(" [%d] %s %d%s", i, vpi_get(vpiType, bit) == vpiPortBit ? "bit" : "other",
                   position(port, bit), same ? "" : " multi differs");
        if (i == low || i == high) continue;
        vpiHandle sub = vpi_handle_by_index(bit, 0);
        if (sub || !failed()) vpi_printf(" (bit by index answers)");
    }
    vpi_printf("\n");
    vpiHandle it = vpi_iterate(vpiBit, port), bit;
    int error = failed();
    if (!it) vpi_printf("  no bits%s\n", error ? " error" : "");
    while (it && (bit = vpi_scan(it)) != NULL) {
        vpi_printf(" ");
        connection("low", vpiLowConn, bit);
        connection("high", vpiHighConn, bit);
        vpi_printf("\n");
    }
}

static vpiHandle port_named(const char* scope, const char* name) {
    vpiHandle it = vpi_iterate(vpiPort, vpi_handle_by_name((PLI_BYTE8*)scope, NULL)), port;
    vpiHandle found = NULL;
    while (it && (port = vpi_scan(it)) != NULL)
        if (!found && !strcmp((const char*)vpi_get_str(vpiName, port), name)) found = port;
    return found;
}

static PLI_INT32 d93(PLI_BYTE8* data) {
    (void)data;
    port_bits(port_named("tb.u", "a"), -1, 4);
    port_bits(port_named("tb.u", "b"), -1, 4);
    port_bits(port_named("tb.u", "c"), 3, 8);
    port_bits(port_named("tb.u", "s"), -1, 1);
    port_bits(port_named("tb.u", "n"), -4, 0);
    port_bits(port_named("tb.l", "x"), 1, 6);
    port_bits(port_named("tb.l", "z"), -1, 3);
    port_bits(port_named("tb.l", "r"), -1, 3);
    vpiHandle m = port_named("tb.u", "m");
    vpiHandle bit = vpi_handle_by_index(m, 0);
    int error = failed();
    vpi_printf("m [0] %s%s\n", bit ? "handle" : "NULL", error ? " error" : "");
    vpiHandle pk = port_named("tb.u", "pk");
    vpi_printf("pk:");
    for (int i = 0; i <= 2; ++i)
        for (int j = -1; j <= 3; ++j) {
            PLI_INT32 indices[2] = {i, j};
            bit = vpi_handle_by_multi_index(pk, 2, indices);
            error = failed();
            if (bit) vpi_printf(" [%d][%d] %d", i, j, position(pk, bit));
            else vpi_printf(" [%d][%d] NULL%s", i, j, error ? " error" : "");
        }
    bit = vpi_handle_by_index(pk, 1);
    error = failed();
    vpi_printf("\npk [1] %s%s\n", bit ? "handle" : "NULL", error ? " error" : "");
    bit = vpi_handle_by_multi_index(pk, 2, (PLI_INT32[]){1, 2});
    vpi_printf("pk [1][2]");
    connection("low", vpiLowConn, bit);
    connection("high", vpiHighConn, bit);
    vpi_printf("\n");
    return 0;
}

static void register_d93(void) {
    s_vpi_systf_data task = {vpiSysTask, 0, (PLI_BYTE8*)"$d93", d93, NULL, NULL, NULL};
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_d93, NULL};
