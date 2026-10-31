/* Decision A19-D48: nets of a declared data type.
 * Companion of A19-D48_typed_nets.sv. */
#include "sv_vpi_user.h"
#include <stdio.h>
#include <string.h>

/* The error level of the last call, clearing it. */
static int level(void) {
    s_vpi_error_info info;
    return vpi_chk_error(&info) ? (int)info.level : 0;
}

/* One net: its type, net type, size, sign, vpiPackedArrayMember, vpiVector,
 * typespec type and value (each with the error level of its call), then
 * the count and first object of its vpiMember, vpiElement, vpiBit and
 * vpiRange iterations. */
static void show(vpiHandle net) {
    static const int relations[] = {vpiMember, vpiElement, vpiBit, vpiRange};
    char name[64];
    const char* text = (const char*)vpi_get_str(vpiName, net);
    snprintf(name, sizeof name, "%s", text ? text : "?");
    int type = (int)vpi_get(vpiType, net);
    level();
    int net_type = (int)vpi_get(vpiNetType, net);
    int net_type_level = level();
    int size = (int)vpi_get(vpiSize, net);
    level();
    int is_signed = (int)vpi_get(vpiSigned, net);
    level();
    int member = (int)vpi_get(vpiPackedArrayMember, net);
    int member_level = level();
    int vector = (int)vpi_get(vpiVector, net);
    int vector_level = level();
    vpiHandle typespec = vpi_handle(vpiTypespec, net);
    int typespec_level = level();
    s_vpi_value value;
    memset(&value, 0, sizeof value);
    value.format = vpiBinStrVal;
    vpi_get_value(net, &value);
    int value_level = level();
    vpi_printf("%s type=%d nettype=%d/%d size=%d signed=%d pam=%d/%d vec=%d/%d ts=%d/%d val=%s/%d\n",
               name, type, net_type, net_type_level, size, is_signed, member, member_level, vector,
               vector_level, typespec ? (int)vpi_get(vpiType, typespec) : 0, typespec_level,
               value_level ? "-" : value.value.str, value_level);
    for (size_t k = 0; k < sizeof relations / sizeof relations[0]; ++k) {
        vpiHandle it = vpi_iterate(relations[k], net);
        int iterate_level = level();
        int count = 0;
        char first[80] = "";
        vpiHandle object;
        while (it && (object = vpi_scan(it))) {
            if (!count) {
                const char* object_name = (const char*)vpi_get_str(vpiName, object);
                snprintf(first, sizeof first, "%s:%d", object_name ? object_name : "?",
                         (int)vpi_get(vpiType, object));
            }
            ++count;
        }
        vpi_printf("  iter %d: n=%d level=%d first=%s\n", relations[k], count, iterate_level,
                   first);
    }
}

static PLI_INT32 d48(PLI_BYTE8* data) {
    (void)data;
    vpiHandle scope = vpi_handle_by_name("tb", NULL);
    vpiHandle it = vpi_iterate(vpiNet, scope), object;
    while (it && (object = vpi_scan(it))) show(object);
    it = vpi_iterate(vpiNetArray, scope);
    while (it && (object = vpi_scan(it))) {
        vpi_printf("array %s type=%d\n", (const char*)vpi_get_str(vpiName, object),
                   (int)vpi_get(vpiType, object));
        vpiHandle element = vpi_handle_by_index(object, 1);
        level();
        if (element) show(element);
    }
    return 0;
}

static void startup(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof task);
    task.type = vpiSysTask;
    task.tfname = "$d48";
    task.calltf = d48;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
