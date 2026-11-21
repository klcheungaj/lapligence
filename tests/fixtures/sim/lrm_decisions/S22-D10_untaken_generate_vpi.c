/* Companion of S22-D10_untaken_generate_vpi.sv: a VPI application
 * (vlog_startup_routines) with the system task $p and an application
 * registered as $random, which nothing calls. */
#include "sv_vpi_user.h"
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

static PLI_INT32 p(PLI_BYTE8* data) {
    static const char* names[] = {"tb.or$1", "tb.or$2", "tb.or$3", "tb.u",   "tb.u.x",
                                  "tb.u.n",  "tb.u.blk", "tb.c0",  "tb.t.z"};
    (void)data;
    for (int i = 0; i < (int)(sizeof(names) / sizeof(names[0])); ++i) {
        vpiHandle found = vpi_handle_by_name((PLI_BYTE8*)names[i], NULL);
        vpi_chk_error(NULL);
        vpi_printf("by name %s: %s\n", names[i],
                   found ? text(vpi_get_str(vpiFullName, found)) : "NULL");
    }
    return 0;
}

static PLI_INT32 random_calltf(PLI_BYTE8* data) {
    s_vpi_value value;
    (void)data;
    vpi_printf("application $random called\n");
    value.format = vpiIntVal;
    value.value.integer = 4;
    vpi_put_value(vpi_handle(vpiSysTfCall, NULL), &value, NULL, vpiNoDelay);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof(data));
    data.type = vpiSysTask;
    data.tfname = (PLI_BYTE8*)"$p";
    data.calltf = p;
    vpi_register_systf(&data);
    memset(&data, 0, sizeof(data));
    data.type = vpiSysFunc;
    data.sysfunctype = vpiIntFunc;
    data.tfname = (PLI_BYTE8*)"$random";
    data.calltf = random_calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
