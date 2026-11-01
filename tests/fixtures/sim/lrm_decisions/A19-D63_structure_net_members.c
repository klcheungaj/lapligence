/* Decision A19-D63: the members of a structure or union net.
 * Companion of A19-D63_structure_net_members.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

static int failed(void) {
    s_vpi_error_info error;
    return vpi_chk_error(&error) != 0;
}

static void member(vpiHandle m, int depth) {
    s_vpi_value v;
    memset(&v, 0, sizeof v);
    v.format = vpiBinStrVal;
    vpi_get_value(m, &v);
    int error = failed();
    /* Strings from vpi_get_str and vpi_get_value last until the next call. */
    char value[64], name[64], parent_name[64];
    snprintf(value, sizeof value, "%s", error ? "-" : v.value.str);
    snprintf(name, sizeof name, "%s", (const char*)vpi_get_str(vpiFullName, m));
    vpiHandle parent = vpi_handle(vpiParent, m);
    snprintf(parent_name, sizeof parent_name, "%s",
             parent ? (const char*)vpi_get_str(vpiFullName, parent) : "NULL");
    vpi_printf("%*s%s %s size=%d value=%s parent=%s member=%d nettype=%d\n", depth * 2, "",
               (const char*)vpi_get_str(vpiType, m), name, (int)vpi_get(vpiSize, m), value,
               parent_name, (int)vpi_get(vpiStructUnionMember, m), (int)vpi_get(vpiNetType, m));
    vpiHandle named = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    vpi_printf("%*s  by name %s\n", depth * 2, "", named && vpi_compare_objects(named, m) ? "same" : "different");
    vpiHandle it = vpi_iterate(vpiMember, m), x;
    (void)failed();
    while (it && (x = vpi_scan(it))) member(x, depth + 1);
}

static void net(vpiHandle n) {
    vpi_printf("%s %s\n", (const char*)vpi_get_str(vpiType, n), (const char*)vpi_get_str(vpiFullName, n));
    vpiHandle it = vpi_iterate(vpiMember, n), m;
    int error = failed();
    if (!it) vpi_printf("  no members%s\n", error ? "(error)" : "");
    while (it && (m = vpi_scan(it))) member(m, 1);
}

static PLI_INT32 d63(PLI_BYTE8* data) {
    (void)data;
    static const char* const names[] = {"tb.sn", "tb.un", "tb.on"};
    for (unsigned i = 0; i < sizeof names / sizeof names[0]; ++i)
        net(vpi_handle_by_name((PLI_BYTE8*)names[i], NULL));
    vpiHandle array = vpi_handle_by_name((PLI_BYTE8*)"tb.sarr", NULL);
    net(vpi_handle_by_index(array, 1));
    return 0;
}

static void register_d63(void) {
    s_vpi_systf_data task = {vpiSysTask, 0, (PLI_BYTE8*)"$d63", d63, NULL, NULL, NULL};
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {register_d63, NULL};
