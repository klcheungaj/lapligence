/* Companion of S43-D4_one_shot_callback_handle.sv: a VPI application. */
#include "vpi_user.h"
#include <string.h>

static vpiHandle first, start_handle;

static vpiHandle after(unsigned delay, PLI_INT32 (*routine)(p_cb_data)) {
    s_vpi_time t;
    s_cb_data cb;
    memset(&t, 0, sizeof(t));
    memset(&cb, 0, sizeof(cb));
    t.type = vpiSimTime;
    t.low = delay;
    cb.reason = cbAfterDelay;
    cb.cb_rtn = routine;
    cb.time = &t;
    return vpi_register_cb(&cb);
}

static PLI_INT32 ran(p_cb_data data) {
    vpi_printf("first ran at %u\n", (unsigned)data->time->low);
    return 0;
}

static PLI_INT32 check(p_cb_data data) {
    (void)data;
    PLI_INT32 removed = vpi_remove_cb(first);
    PLI_INT32 level = vpi_chk_error(NULL);
    vpi_printf("remove the callback that ran: %d, error %d\n", (int)removed, (int)level);
    s_cb_data info;
    memset(&info, 0, sizeof(info));
    vpi_get_cb_info(start_handle, &info);
    vpi_printf("info of cbStartOfSimulation after it ran: error %d\n", (int)vpi_chk_error(NULL));
    s_cb_data late;
    memset(&late, 0, sizeof(late));
    late.reason = cbStartOfSimulation;
    late.cb_rtn = ran;
    vpiHandle handle = vpi_register_cb(&late);
    level = vpi_chk_error(NULL);
    vpi_printf("late cbStartOfSimulation: %s, error %d\n", handle ? "handle" : "NULL", (int)level);
    return 0;
}

static PLI_INT32 start(p_cb_data data) {
    (void)data;
    first = after(1, ran);
    vpiHandle released = after(2, ran);
    vpi_printf("release a pending callback handle: %d\n", (int)vpi_release_handle(released));
    after(3, check);
    return 0;
}

static void startup(void) {
    s_cb_data cb;
    memset(&cb, 0, sizeof(cb));
    cb.reason = cbStartOfSimulation;
    cb.cb_rtn = start;
    start_handle = vpi_register_cb(&cb);
}

void (*vlog_startup_routines[])(void) = {startup, 0};
