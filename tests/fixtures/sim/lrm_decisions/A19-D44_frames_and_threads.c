/* Decision A19-D44: frames and threads (see the .sv). $d44(n) prints the
 * frame chain and the threads seen from step n, places the frame and thread
 * callbacks and queries a frame and a thread after they ended. */
#include "sv_vpi_user.h"
#include <string.h>

static vpiHandle held_frame, held_thread;

static int failed(void) { return vpi_chk_error(NULL) != 0; }

static const char* name_of(vpiHandle h) {
    const char* name = h ? vpi_get_str(vpiFullName, h) : NULL;
    return name ? name : "-";
}

static PLI_INT32 seen(p_cb_data data) {
    vpi_printf("  cb %s obj=%s\n", (const char*)data->user_data,
               data->obj ? vpi_get_str(vpiType, data->obj) : "NULL");
    return 0;
}

static void place(PLI_INT32 reason, vpiHandle obj, const char* tag) {
    s_cb_data data;
    s_vpi_time time;
    memset(&data, 0, sizeof data);
    time.type = vpiSuppressTime;
    data.reason = reason;
    data.cb_rtn = seen;
    data.obj = obj;
    data.time = &time;
    data.user_data = (PLI_BYTE8*)tag;
    vpiHandle cb = vpi_register_cb(&data);
    int error = failed();
    vpi_printf("  place %s: %s (error %d)\n", tag, cb ? "ok" : "NULL", error);
}

static PLI_INT32 calltf(PLI_BYTE8* user_data) {
    (void)user_data;
    vpiHandle call = vpi_handle(vpiSysTfCall, NULL);
    vpiHandle args = vpi_iterate(vpiArgument, call);
    vpiHandle arg = vpi_scan(args);
    vpi_free_object(args);
    s_vpi_value value;
    value.format = vpiIntVal;
    vpi_get_value(arg, &value);
    int step = value.value.integer;
    vpiHandle frame = vpi_handle(vpiFrame, NULL);
    int error = failed();
    vpi_printf("step %d: frame %s (error %d)", step, name_of(frame), error);
    for (vpiHandle f = frame; f; f = vpi_handle(vpiParent, f)) {
        int active = (int)vpi_get(vpiActive, f);
        int active_error = failed();
        vpiHandle stmt = vpi_handle(vpiStmt, f);
        vpi_printf(" [%s active %d/%d stmt %s]", name_of(f), active, active_error,
                   stmt ? vpi_get_str(vpiType, stmt) : "NULL");
    }
    vpi_printf("\n");
    vpiHandle it = vpi_iterate(vpiThread, NULL), thread;
    int count = 0;
    while (it && (thread = vpi_scan(it))) {
        int active = (int)vpi_get(vpiActive, thread);
        vpiHandle origin = vpi_handle(vpiOrigin, thread);
        int origin_error = failed();
        vpiHandle parent = vpi_handle(vpiParent, thread);
        vpi_printf("  thread %d: active %d origin %s/%d parent %s\n", count++, active,
                   origin ? "stmt" : "NULL", origin_error, parent ? "thread" : "NULL");
        if (parent) held_thread = thread;
    }
    if (held_frame && step != 1) {
        int active = (int)vpi_get(vpiActive, held_frame);
        int stale = failed();
        vpi_printf("  ended frame: active %d (error %d)\n", active, stale);
    }
    if (held_thread && step == 5) {
        int active = (int)vpi_get(vpiActive, held_thread);
        int stale = failed();
        vpi_printf("  ended thread: active %d (error %d)\n", active, stale);
    }
    if (step == 0) {
        place(cbStartOfThread, NULL, "start-of-thread");
        place(cbStartOfFrame, NULL, "start-of-frame-null");
        place(cbStartOfFrame, vpi_handle_by_name("tb.t2", NULL), "start-of-frame-t2");
        place(cbEndOfThread, NULL, "end-of-thread-null");
    }
    if (step == 1) {
        held_frame = frame;
        place(cbEndOfFrame, frame, "end-of-frame-t1");
    }
    if (step == 4) {
        vpiHandle event = vpi_handle_by_name("tb.ev", NULL);
        vpiHandle waiting = vpi_iterate(vpiWaitingProcesses, event);
        int waiters = 0;
        while (waiting && vpi_scan(waiting)) ++waiters;
        vpi_printf("  threads waiting on tb.ev: %d\n", waiters);
        place(cbEndOfThread, held_thread, "end-of-thread-branch");
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = "$d44";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
