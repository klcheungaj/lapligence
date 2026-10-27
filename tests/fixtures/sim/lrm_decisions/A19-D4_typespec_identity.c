/* Companion of A19-D4_typespec_identity.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static vpiHandle typespec(const char* name) {
    return vpi_handle(vpiTypespec, vpi_handle_by_name((PLI_BYTE8*)name, NULL));
}

static void compare(const char* left, const char* right) {
    vpi_printf("%s %s: %d\n", left, right, (int)vpi_compare_objects(typespec(left), typespec(right)));
}

static PLI_INT32 d4(PLI_BYTE8* data) {
    (void)data;
    compare("tb.n1", "tb.n1");
    compare("tb.n1", "tb.n2");
    compare("tb.x", "tb.y");
    compare("tb.n1", "tb.o");
    compare("tb.n1", "tb.x");
    compare("tb.x", "tb.z");
    return 0;
}

static void startup(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = "$d4";
    data.calltf = d4;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {startup, NULL};
