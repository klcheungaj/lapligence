/* Companion of A19-D3_unpacked_struct_elements.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

static PLI_INT32 d3(PLI_BYTE8* data) {
    (void)data;
    vpiHandle element = vpi_handle_by_index(vpi_handle_by_name("tb.arr", NULL), 1);
    vpi_printf("%s %s size=%d\n", text(vpi_get_str(vpiType, element)),
               text(vpi_get_str(vpiFullName, element)), (int)vpi_get(vpiSize, element));
    s_vpi_value value;
    value.format = vpiHexStrVal;
    vpi_get_value(element, &value);
    vpi_printf("get element level=%d\n", (int)vpi_chk_error(NULL));
    value.format = vpiIntVal;
    value.value.integer = 0;
    vpi_put_value(element, &value, NULL, vpiNoDelay);
    vpi_printf("put element level=%d\n", (int)vpi_chk_error(NULL));
    vpiHandle members = vpi_iterate(vpiMember, element), member;
    while ((member = vpi_scan(members))) {
        value.format = vpiHexStrVal;
        vpi_get_value(member, &value);
        vpi_printf("member %s = %s\n", text(vpi_get_str(vpiName, member)), value.value.str);
    }
    vpiHandle b = vpi_handle_by_name("tb.arr[1].b", NULL);
    value.format = vpiIntVal;
    value.value.integer = 0x5a;
    vpi_put_value(b, &value, NULL, vpiNoDelay);
    vpi_printf("put %s level=%d\n", text(vpi_get_str(vpiFullName, b)), (int)vpi_chk_error(NULL));
    return 0;
}

static void startup(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = "$d3";
    data.calltf = d3;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
