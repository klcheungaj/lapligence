/* SIM-042 delayed vpi_put_value scheduling (decision S42-D5): the delay
 * modes as Active-region events, cancellation in each mode, and a put still
 * pending at $finish. With SIM042_PUT_STOP set, a cbAfterDelay callback
 * stops the run at time 4 (vpiStop) while puts are pending. Companion of
 * put_region.sv. */
#include "vpi_user.h"
#include <stdlib.h>
#include <string.h>

static vpiHandle put(const char* name, PLI_INT32 integer, unsigned delay, PLI_INT32 flags) {
    s_vpi_value v;
    memset(&v, 0, sizeof(v));
    v.format = vpiIntVal;
    v.value.integer = integer;
    s_vpi_time t;
    memset(&t, 0, sizeof(t));
    t.type = vpiSimTime;
    t.low = delay;
    vpiHandle handle = vpi_handle_by_name((PLI_BYTE8*)name, NULL);
    if (!handle) vpi_printf("FAIL lookup %s\n", name);
    vpiHandle event = vpi_put_value(handle, &v, &t, flags);
    if (vpi_chk_error(NULL)) vpi_printf("FAIL put %s=%d\n", name, (int)integer);
    return event;
}

static void cancel(vpiHandle event) {
    if (!event) vpi_printf("FAIL no event handle\n");
    vpi_put_value(event, NULL, NULL, vpiCancelEvent);
    if (vpi_chk_error(NULL)) vpi_printf("FAIL cancel\n");
}

static PLI_INT32 stop_now(p_cb_data data) {
    (void)data;
    vpi_printf("4 stop\n");
    vpi_control(vpiStop, 1);
    return 0;
}

static PLI_INT32 put_region(PLI_BYTE8* data) {
    (void)data;
    put("tb.d", 5, 2, vpiInertialDelay);
    put("tb.e", 6, 2, vpiTransportDelay);
    put("tb.f", 7, 2, vpiPureTransportDelay);
    /* Inertial: a cancelled event, then 2@5 removed by 3@6. */
    cancel(put("tb.ci", 1, 4, vpiInertialDelay | vpiReturnEvent));
    put("tb.ci", 2, 5, vpiInertialDelay);
    put("tb.ci", 3, 6, vpiInertialDelay);
    /* Transport: 1@6 removed by the earlier 2@5; 3@7 cancelled. */
    put("tb.ct", 1, 6, vpiTransportDelay);
    put("tb.ct", 2, 5, vpiTransportDelay);
    cancel(put("tb.ct", 3, 7, vpiTransportDelay | vpiReturnEvent));
    /* Pure transport: nothing removed; 3@5 cancelled. */
    put("tb.cp", 1, 4, vpiPureTransportDelay);
    put("tb.cp", 2, 3, vpiPureTransportDelay);
    cancel(put("tb.cp", 3, 5, vpiPureTransportDelay | vpiReturnEvent));
    /* Still pending when $finish ends the run at 9. */
    put("tb.late", 8, 12, vpiPureTransportDelay);
    vpi_printf("0 scheduled\n");
    if (getenv("SIM042_PUT_STOP")) {
        s_vpi_time t;
        memset(&t, 0, sizeof(t));
        t.type = vpiSimTime;
        t.low = 4;
        s_cb_data cb;
        memset(&cb, 0, sizeof(cb));
        cb.reason = cbAfterDelay;
        cb.cb_rtn = stop_now;
        cb.time = &t;
        vpi_register_cb(&cb);
    }
    return 0;
}

static void register_all(void) {
    s_vpi_systf_data t;
    memset(&t, 0, sizeof(t));
    t.type = vpiSysTask;
    t.tfname = (PLI_BYTE8*)"$put_region";
    t.calltf = put_region;
    vpi_register_systf(&t);
}

void (*vlog_startup_routines[])(void) = {register_all, NULL};
