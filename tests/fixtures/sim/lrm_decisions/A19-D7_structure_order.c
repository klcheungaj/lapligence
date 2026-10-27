/* Companion of A19-D7_structure_order.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static const char* text(const char* s) { return s ? s : "NULL"; }

static PLI_INT32 d7(PLI_BYTE8* data) {
    (void)data;
    vpiHandle tb = vpi_handle_by_name("tb", NULL), h;
    vpiHandle it = vpi_iterate(vpiProcess, tb);
    while ((h = vpi_scan(it))) {
        const char* name = vpi_get_str(vpiName, h);
        vpi_printf("process %s name=%s(%d)\n", text(vpi_get_str(vpiType, h)), text(name),
                   (int)vpi_chk_error(NULL));
    }
    it = vpi_iterate(vpiContAssign, tb);
    while ((h = vpi_scan(it)))
        vpi_printf("assign %s\n", text(vpi_get_str(vpiFullName, vpi_handle(vpiLhs, h))));
    vpiHandle array = vpi_handle_by_name("tb.c", NULL);
    vpi_printf("%s %s:", text(vpi_get_str(vpiType, array)), text(vpi_get_str(vpiFullName, array)));
    it = vpi_iterate(vpiModule, array);
    while ((h = vpi_scan(it))) vpi_printf(" %s", text(vpi_get_str(vpiName, h)));
    vpi_printf("\n");
    vpiHandle module = vpi_handle_by_name("tb.c[3]", NULL);
    it = vpi_iterate(vpiPort, module);
    while ((h = vpi_scan(it))) {
        vpi_printf("port %s", text(vpi_get_str(vpiName, h)));
        if (vpi_get(vpiPortType, h) == vpiModportPort) {
            vpiHandle low = vpi_handle(vpiLowConn, h), high = vpi_handle(vpiHighConn, h);
            vpi_printf(" low=%s:%s high=%s:%s actual=%s:%s\n", text(vpi_get_str(vpiType, low)),
                       text(vpi_get_str(vpiName, low)), text(vpi_get_str(vpiType, high)),
                       text(vpi_get_str(vpiName, high)),
                       text(vpi_get_str(vpiType, vpi_handle(vpiActual, high))),
                       text(vpi_get_str(vpiFullName, vpi_handle(vpiActual, low))));
            continue;
        }
        vpiHandle bits = vpi_iterate(vpiBit, h), bit;
        while ((bit = vpi_scan(bits))) {
            vpi_printf(" %s", text(vpi_get_str(vpiFullName, vpi_handle(vpiLowConn, bit))));
            const char* name = vpi_get_str(vpiName, bit);
            vpi_printf("(name=%s,%d)", text(name), (int)vpi_chk_error(NULL));
        }
        vpi_printf("\n");
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$d7";
    task.calltf = d7;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
