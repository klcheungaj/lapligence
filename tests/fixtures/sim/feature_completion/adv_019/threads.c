/* ADV-019 threads and frames walker (IEEE 1800-2009 37.25, 37.39, 37.40,
 * 38.36.1). $threads(n) prints the frames and threads seen from step n and
 * places the frame and thread callbacks; held handles to ended frames and
 * threads must fail with an error. */
#include "sv_vpi_user.h"
#include <string.h>

static vpiHandle held_frame, held_thread, main_thread, enter_cb;


static int failed(void) { return vpi_chk_error(NULL) != 0; }

static const char* name_of(vpiHandle frame) {
    const char* name = frame ? vpi_get_str(vpiFullName, frame) : NULL;
    return name ? name : "-";
}

static PLI_INT32 seen(p_cb_data data) {
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("  cb %s t=%u obj=%s", (const char*)data->user_data, now.low,
               data->obj ? vpi_get_str(vpiType, data->obj) : "NULL");
    if (data->reason == cbStartOfFrame) {
        vpiHandle frame = vpi_handle(vpiFrame, NULL);
        vpi_printf(" frame=%s", name_of(frame));
    }
    vpi_printf("\n");
    return 0;
}

static vpiHandle place(PLI_INT32 reason, vpiHandle obj, const char* tag) {
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
    return cb;
}

static void frames(void) {
    vpiHandle frame = vpi_handle(vpiFrame, NULL);
    int error = failed();
    vpi_printf("  frames (error %d):", error);
    while (frame) {
        int active = (int)vpi_get(vpiActive, frame);
        vpiHandle stmt = vpi_handle(vpiStmt, frame);
        vpi_printf(" %s(active %d, stmt %s)", name_of(frame), active,
                   stmt ? vpi_get_str(vpiType, stmt) : "NULL");
        frame = vpi_handle(vpiParent, frame);
    }
    vpi_printf("\n");
}

static void threads(void) {
    vpiHandle it = vpi_iterate(vpiThread, NULL), thread;
    vpi_printf("  threads:");
    while (it && (thread = vpi_scan(it))) {
        vpiHandle parent = vpi_handle(vpiParent, thread);
        vpiHandle origin = vpi_handle(vpiOrigin, thread);
        int origin_error = failed();
        vpiHandle frame = vpi_handle(vpiFrame, thread);
        vpiHandle children = vpi_iterate(vpiThread, thread), child;
        int count = 0;
        while (children && (child = vpi_scan(children))) ++count;
        vpi_printf(" [active %d parent %s origin %s/%d frame %s children %d]",
                   (int)vpi_get(vpiActive, thread), parent ? "thread" : "NULL",
                   origin ? "stmt" : "NULL", origin_error, name_of(frame), count);
    }
    vpi_printf("\n");
}

static void stale(void) {
    if (held_frame) {
        int active = (int)vpi_get(vpiActive, held_frame);
        int error = failed();
        vpi_printf("  held frame: active %d error %d\n", active, error);
    }
    if (held_thread) {
        vpiHandle parent = vpi_handle(vpiParent, held_thread);
        int error = failed();
        vpi_printf("  held thread: parent %s error %d\n", parent ? "thread" : "NULL", error);
    }
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
    s_vpi_time now;
    now.type = vpiSimTime;
    vpi_get_time(NULL, &now);
    vpi_printf("step %d t=%u\n", step, now.low);
    frames();
    threads();
    stale();
    vpiHandle frame = vpi_handle(vpiFrame, NULL);
    if (step == 0) {
        place(cbStartOfFrame, NULL, "sof-null");
        place(cbStartOfFrame, vpi_handle_by_name("tb.t1", NULL), "sof-t1");
        place(cbEndOfFrame, NULL, "eof-null");
        place(cbStartOfThread, NULL, "sot");
        place(cbEndOfThread, NULL, "eot-null");
        place(cbEndOfThread, vpi_handle_by_name("tb.r", NULL), "eot-var");
    }
    if (step == 1) {
        held_frame = frame;
        main_thread = vpi_handle(vpiThread, frame);
        place(cbEndOfFrame, frame, "eof-t1");
        place(cbStartOfThread, main_thread, "sot-obj");
        enter_cb = place(cbEnterThread, main_thread, "enter-main");
    }
    if (step == 2) {
        vpiHandle event = vpi_handle_by_name("tb.ev", NULL);
        vpiHandle waiting = vpi_iterate(vpiWaitingProcesses, event), thread;
        int count = 0;
        while (waiting && (thread = vpi_scan(waiting))) {
            vpiHandle running = vpi_handle(vpiThread, frame);
            count += vpi_compare_objects(thread, running) ? 100 : 1;
        }
        vpi_printf("  waiting on ev: %d\n", count);
    }
    if (step == 3) vpi_remove_cb(enter_cb);
    if (step == 4) {
        vpiHandle it = vpi_iterate(vpiThread, NULL), thread;
        while (it && (thread = vpi_scan(it)))
            if (vpi_handle(vpiParent, thread)) held_thread = thread;
        place(cbEndOfThread, held_thread, "eot-branch");
        place(cbEnterThread, held_thread, "enter-branch");
        vpiHandle assertions = vpi_iterate(vpiAssertion, held_thread);
        int error = failed();
        vpi_printf("  thread assertions: %s (error %d)\n", assertions ? "list" : "NULL", error);
    }
    return 0;
}

static void registration(void) {
    s_vpi_systf_data data;
    memset(&data, 0, sizeof data);
    data.type = vpiSysTask;
    data.tfname = "$threads";
    data.calltf = calltf;
    vpi_register_systf(&data);
}

void (*vlog_startup_routines[])(void) = {registration, 0};
