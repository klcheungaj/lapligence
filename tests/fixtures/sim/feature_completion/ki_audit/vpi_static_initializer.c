/* Known issue (VPI work during static initialization): $sched, called from a
 * static declaration initializer, schedules an inertial put on tb.x one time
 * unit later and returns 7. Companion of vpi_static_initializer.sv. */
#include "vpi_user.h"
#include <string.h>

static PLI_INT32 sched(PLI_BYTE8* data) {
    (void)data;
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiIntVal;
    v.value.integer = 5;
    s_vpi_time t;
    memset(&t, 0, sizeof(t));
    t.type = vpiSimTime;
    t.low = 1;
    vpi_put_value(vpi_handle_by_name((PLI_BYTE8*)"tb.x", NULL), &v, &t, vpiInertialDelay);
    if (vpi_chk_error(NULL)) vpi_printf("FAIL put\n");
    s_vpi_value result;
    memset(&result, 0, sizeof(result));
    result.format = vpiIntVal;
    result.value.integer = 7;
    vpi_put_value(vpi_handle(vpiSysTfCall, NULL), &result, NULL, vpiNoDelay);
    return 0;
}

static void register_all(void) {
    s_vpi_systf_data t;
    memset(&t, 0, sizeof(t));
    t.type = vpiSysFunc;
    t.sysfunctype = vpiIntFunc;
    t.tfname = (PLI_BYTE8*)"$sched";
    t.calltf = sched;
    vpi_register_systf(&t);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
