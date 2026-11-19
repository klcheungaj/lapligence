/* Companion of vpi_time.sv: a VPI application (vlog_startup_routines). */
#include "sv_vpi_user.h"
#include <string.h>

static PLI_INT32 ts_report(PLI_BYTE8* data) {
    s_vpi_time now;
    vpiHandle top = vpi_handle_by_name("tb", NULL);
    (void)data;
    vpi_printf("tb unit=%d precision=%d\n", (int)vpi_get(vpiTimeUnit, top),
               (int)vpi_get(vpiTimePrecision, top));
    vpi_printf("simulation unit=%d precision=%d\n", (int)vpi_get(vpiTimeUnit, NULL),
               (int)vpi_get(vpiTimePrecision, NULL));
    memset(&now, 0, sizeof(now));
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("sim time=%u\n", (unsigned)now.low);
    memset(&now, 0, sizeof(now));
    now.type = vpiScaledRealTime;
    vpi_get_time(top, &now);
    vpi_printf("tb time=%.3f\n", now.real);
    return 0;
}

static void registration(void) {
    s_vpi_systf_data task;
    memset(&task, 0, sizeof(task));
    task.type = vpiSysTask;
    task.tfname = "$ts_report";
    task.calltf = ts_report;
    vpi_register_systf(&task);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
